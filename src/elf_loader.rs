// ELF executable policy and conversion to simulator memory and symbols.
// Byte decoding and file relationships are owned by the shared ELF view.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;

use crate::elf::{
    EI_OSABI, EM_RISCV, ET_EXEC, ElfFile, PT_LOAD, SHF_ALLOC, SHF_EXECINSTR,
    SHF_WRITE, SHN_ABS, SHT_NOBITS, SHT_PROGBITS, SHT_SYMTAB, STT_FILE,
};
use crate::error::{Result, RiscletError};
use crate::{Machine, memory::Segment};

/// Input source for loading an ELF file.
pub enum ElfInput<'a> {
    File(&'a str),
    Bytes(&'a [u8]),
}

pub fn load_elf(input: ElfInput<'_>) -> Result<Machine> {
    let raw = match input {
        ElfInput::File(filename) => {
            Cow::Owned(fs::read(filename).map_err(|e| {
                RiscletError::io(format!(
                    "failed to read file '{filename}': {e}"
                ))
            })?)
        }
        ElfInput::Bytes(bytes) => Cow::Borrowed(bytes),
    };
    let file = ElfFile::parse(&raw)?;
    validate_executable(&file)?;

    // Sections retain the simulator's region boundaries and permissions.
    // Their initialized bytes come from the first covering LOAD segment.
    let mut segments = Vec::new();
    for section in &file.section_headers {
        if is_unsupported_section_type(section.sh_type) {
            return Err(RiscletError::elf(format!(
                "ELF file contains unsupported section type: {:#x}",
                section.sh_type
            )));
        }
        if !matches!(section.sh_type, SHT_PROGBITS | SHT_NOBITS)
            || section.sh_flags & SHF_ALLOC == 0
            || section.sh_size == 0
        {
            continue;
        }

        let end =
            section.sh_addr.checked_add(section.sh_size).ok_or_else(|| {
                RiscletError::elf("allocated section address overflows".into())
            })?;
        if section.sh_addr == 0 {
            return Err(RiscletError::elf(
                "allocated section starts at address zero".into(),
            ));
        }
        let mut init = Vec::new();
        for header in &file.program_headers {
            if header.p_type != PT_LOAD || section.sh_addr < header.p_vaddr {
                continue;
            }
            let start = (section.sh_addr - header.p_vaddr) as usize;
            let bytes = file.program_data(header)?;
            if start < bytes.len() {
                let len = (section.sh_size as usize).min(bytes.len() - start);
                init.extend_from_slice(&bytes[start..start + len]);
                break;
            }
        }
        segments.push(Segment::new(
            section.sh_addr,
            end,
            section.sh_flags & SHF_WRITE != 0,
            section.sh_flags & SHF_EXECINSTR != 0,
            init,
        ));
    }

    // A symbol table is identified by type, and its names by its link. Keep
    // the simulator's requirement for symbols and its existing name filters.
    let table = file
        .section_headers
        .iter()
        .position(|section| section.sh_type == SHT_SYMTAB)
        .ok_or_else(|| {
            RiscletError::elf("ELF file does not contain a symbol table".into())
        })?;
    let symbols = load_symbols(&file, table)?;
    Ok(Machine::new(
        segments,
        file.header.e_entry,
        symbols.global_pointer,
        symbols.addresses,
        symbols.other,
    ))
}

fn validate_executable(file: &ElfFile<'_>) -> Result<()> {
    let header = &file.header;
    if header.e_ident[7] != EI_OSABI {
        return Err(RiscletError::elf(
            "ELF file OS/ABI is not System V (must be 0)".into(),
        ));
    }
    if header.e_type != ET_EXEC {
        return Err(RiscletError::elf(format!(
            "ELF file is not executable (type={})",
            header.e_type
        )));
    }
    if header.e_machine != EM_RISCV {
        return Err(RiscletError::elf(format!(
            "ELF file is not RISC-V (machine={:#x})",
            header.e_machine
        )));
    }
    if file.program_headers.is_empty() {
        return Err(RiscletError::elf(
            "ELF file has no program headers".into(),
        ));
    }
    Ok(())
}

// Relocations, dynamic linking, and the listed runtime metadata are outside
// the simulator's executable model.
fn is_unsupported_section_type(section_type: u32) -> bool {
    matches!(
        section_type,
        0x4 | 0x5 | 0x6 | 0x9 | 0xb | 0xe | 0xf | 0x10 | 0x11
    )
}

struct LoadedSymbols {
    addresses: HashMap<u32, String>,
    other: HashMap<String, u32>,
    global_pointer: u32,
}

fn load_symbols(file: &ElfFile<'_>, table: usize) -> Result<LoadedSymbols> {
    let mut symbols = LoadedSymbols {
        addresses: HashMap::new(),
        other: HashMap::new(),
        global_pointer: 0,
    };
    for symbol in file.symbols(table)? {
        let name = String::from_utf8_lossy(file.symbol_name(table, &symbol)?)
            .into_owned();
        if name.is_empty() || symbol.symbol_type() == STT_FILE {
            continue;
        }

        // The global pointer survives the internal-name filter because it
        // supplies the simulator's initial gp value.
        if name == "__global_pointer$" {
            symbols.global_pointer = symbol.st_value;
            symbols.addresses.insert(symbol.st_value, name);
            continue;
        }
        if name.starts_with('$') || name.starts_with("__") {
            continue;
        }
        if symbol.st_shndx > 0 && symbol.st_shndx != SHN_ABS {
            symbols.addresses.insert(symbol.st_value, name);
        } else {
            symbols.other.insert(name, symbol.st_value);
        }
    }
    Ok(symbols)
}
