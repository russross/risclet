// ELF executable policy and conversion to simulator memory and symbols.
// Byte decoding and file relationships are owned by the shared ELF view.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;

use crate::elf::{
    EF_RISCV_FLOAT_ABI, EF_RISCV_FLOAT_ABI_SOFT, EI_OSABI, EM_RISCV, ET_EXEC,
    ElfFile, PF_W, PF_X, PT_LOAD, SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE, SHN_ABS,
    SHN_UNDEF, SHT_NOBITS, SHT_PROGBITS, SHT_SYMTAB, STT_FILE,
};
use crate::error::{
    Diagnostic, DiagnosticContext, Result, RiscletError, address,
};
use crate::{
    Machine,
    memory::{MemoryLayout, Segment},
};

/// Input source for loading an ELF file.
pub enum ElfInput<'a> {
    File(&'a str),
    Bytes(&'a [u8]),
}

pub fn load_elf(input: ElfInput<'_>) -> Result<Machine> {
    load_elf_with_style(input, false)
}

pub fn load_elf_with_style(input: ElfInput<'_>, hex: bool) -> Result<Machine> {
    let name = match &input {
        ElfInput::File(filename) => format!("executable '{filename}'"),
        ElfInput::Bytes(_) => "assembled executable".into(),
    };
    load_elf_inner(input, hex).map_err(|error| match error {
        RiscletError::Elf(message) => {
            let mut diagnostic = Diagnostic::from(message);
            diagnostic.context =
                DiagnosticContext::Operation(format!("while loading {name}"));
            RiscletError::Diagnostic(Box::new(diagnostic))
        }
        other => other,
    })
}

fn load_elf_inner(input: ElfInput<'_>, hex: bool) -> Result<Machine> {
    let raw = match input {
        ElfInput::File(filename) => {
            Cow::Owned(fs::read(filename).map_err(|e| {
                RiscletError::io_at("read executable", filename, e)
            })?)
        }
        ElfInput::Bytes(bytes) => Cow::Borrowed(bytes),
    };
    let file = ElfFile::parse(&raw)?;
    validate_executable(&file, hex)?;

    // Sections retain the simulator's region boundaries and permissions.
    // Their initialized bytes come from the first covering LOAD segment.
    let mut segments = Vec::new();
    for (index, section) in file.section_headers.iter().enumerate() {
        let context = format!(
            "section {index} '{}' at address {}, size {}",
            String::from_utf8_lossy(file.section_name(index)?),
            address(section.sh_addr, hex),
            address(section.sh_size, hex)
        );
        if is_unsupported_section_type(section.sh_type) {
            return Err(RiscletError::elf(format!(
                "{context}: ELF file contains unsupported section type: {:#x}",
                section.sh_type
            )));
        }
        if section.sh_flags & SHF_ALLOC != 0
            && (!matches!(section.sh_type, SHT_PROGBITS | SHT_NOBITS)
                || section.sh_flags & 0x400 != 0)
        {
            return Err(RiscletError::elf(format!(
                "{context}: unsupported allocated section type or thread-local storage"
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
                RiscletError::elf(format!(
                    "{context}: allocated section address overflows"
                ))
            })?;
        if section.sh_addr == 0 {
            return Err(RiscletError::elf(format!(
                "{context}: allocated section starts at address zero"
            )));
        }
        let overlaps = file.section_headers.iter().any(|other| {
            !std::ptr::eq(section, other)
                && other.sh_flags & SHF_ALLOC != 0
                && other.sh_size != 0
                && u64::from(section.sh_addr)
                    < u64::from(other.sh_addr) + u64::from(other.sh_size)
                && other.sh_addr < end
        });
        if overlaps {
            return Err(RiscletError::elf(format!(
                "{context}: allocated sections overlap; inspect allocated section address ranges"
            )));
        }
        let header = file
            .program_headers
            .iter()
            .find(|header| {
                header.p_type == PT_LOAD
                    && section.sh_addr >= header.p_vaddr
                    && end <= header.p_vaddr + header.p_memsz
            })
            .ok_or_else(|| {
                RiscletError::elf(
                    format!("{context}: allocated section is not covered by a LOAD segment"),
                )
            })?;

        // Initialized sections must agree with the segment's file mapping.
        // NOBITS sections always start zero-filled, even at a file boundary.
        if (section.sh_flags & SHF_WRITE != 0 && header.p_flags & PF_W == 0)
            || (section.sh_flags & SHF_EXECINSTR != 0
                && header.p_flags & PF_X == 0)
        {
            return Err(RiscletError::elf(format!(
                "{context}: section permissions 0x{:x} exceed LOAD permissions 0x{:x}",
                section.sh_flags, header.p_flags
            )));
        }
        let init = if section.sh_type == SHT_NOBITS {
            Vec::new()
        } else {
            let start = section.sh_addr - header.p_vaddr;
            if u64::from(start) + u64::from(section.sh_size)
                > u64::from(header.p_filesz)
                || u64::from(header.p_offset) + u64::from(start)
                    != u64::from(section.sh_offset)
            {
                return Err(RiscletError::elf(format!(
                    "{context}: initialized section disagrees with LOAD file mapping; section offset {}, LOAD offset {}, LOAD file size {}",
                    address(section.sh_offset, hex),
                    address(header.p_offset, hex),
                    address(header.p_filesz, hex)
                )));
            }
            file.program_data(header)?
                [start as usize..(start + section.sh_size) as usize]
                .to_vec()
        };
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
            RiscletError::elf("ELF file does not contain a symbol table; Risclet requires section and symbol metadata, so use an unstripped executable".into())
        })?;
    let symbols = load_symbols(&file, table)?;
    // Stack placement must not shadow any section in the simulated address space.
    let memory_layout = MemoryLayout::new(&segments);
    if file.section_headers.iter().any(|section| {
        section.sh_flags & SHF_ALLOC != 0
            && section.sh_size != 0
            && section.sh_addr < memory_layout.stack_end
            && u64::from(section.sh_addr) + u64::from(section.sh_size)
                > u64::from(memory_layout.stack_start)
    }) {
        return Err(RiscletError::elf(
            "ELF memory layout leaves no room for the simulator stack".into(),
        ));
    }
    Ok(Machine::new(
        segments,
        file.header.e_entry,
        symbols.global_pointer,
        symbols.addresses,
        symbols.other,
    ))
}

fn validate_executable(file: &ElfFile<'_>, hex: bool) -> Result<()> {
    let header = &file.header;
    if header.e_ident[7] != EI_OSABI {
        return Err(RiscletError::elf(format!(
            "ELF file OS/ABI is not System V (must be 0); got {}",
            header.e_ident[7]
        )));
    }
    if header.e_flags & EF_RISCV_FLOAT_ABI != EF_RISCV_FLOAT_ABI_SOFT {
        return Err(RiscletError::elf(format!(
            "ELF file must use the soft-float ABI; flags are 0x{:x}, floating-point ABI bits must be zero",
            header.e_flags
        )));
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
    if !header.e_entry.is_multiple_of(2)
        || !file.section_headers.iter().any(|section| {
            section.sh_type == SHT_PROGBITS
                && section.sh_flags & (SHF_ALLOC | SHF_EXECINSTR)
                    == (SHF_ALLOC | SHF_EXECINSTR)
                && header.e_entry >= section.sh_addr
                && u64::from(header.e_entry) + 2
                    <= u64::from(section.sh_addr) + u64::from(section.sh_size)
        })
    {
        return Err(RiscletError::elf(format!(
            "ELF entry must be aligned and inside executable code; entry address is {}",
            address(header.e_entry, hex)
        )));
    }
    if file
        .program_headers
        .iter()
        .any(|header| matches!(header.p_type, 2 | 3 | 7))
    {
        return Err(RiscletError::elf(
            "dynamic linking and thread-local storage are not supported".into(),
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
        if name.is_empty()
            || symbol.symbol_type() == STT_FILE
            || symbol.st_shndx == SHN_UNDEF
        {
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
