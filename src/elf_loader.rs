// ELF binary format loading for the RISC-V simulator
//
// This module loads ELF executables into memory for simulation.
// All byte-level parsing is delegated to the elf module's decode methods.

use std::collections::HashMap;

use crate::elf::{
    ElfHeader, ElfProgramHeader, ElfSectionHeader, ElfSymbol, PT_LOAD,
    SHT_STRTAB, SHT_SYMTAB, STT_FILE, SYMBOL_ENTRY_SIZE, StringTable,
};
use crate::error::Result;
use crate::{Machine, memory::Segment};

/// Input source for loading an ELF file
pub enum ElfInput<'a> {
    /// Load from a file path
    File(&'a str),
    /// Load from a byte slice
    Bytes(&'a [u8]),
}

/// Load an ELF file from either a filesystem path or a byte slice
pub fn load_elf(input: ElfInput) -> Result<Machine> {
    let raw = match input {
        ElfInput::File(filename) => std::fs::read(filename).map_err(|e| {
            format!("failed to read file '{}': {}", filename, e)
        })?,
        ElfInput::Bytes(bytes) => bytes.to_vec(),
    };

    // Validate minimum size for ELF header
    if raw.len() < 52 {
        return Err("ELF data is too short to contain a valid header".into());
    }

    // Validate ELF magic number
    if raw[0..4] != *b"\x7fELF" {
        return Err(
            "ELF data does not have valid ELF magic number (0x7f 'E' 'L' 'F')"
                .into(),
        );
    }

    // Validate ELF class (32-bit), data (little-endian), version, and OS/ABI
    if raw[4] != 1 {
        return Err("ELF file is not 32-bit (class must be 1)".into());
    }
    if raw[5] != 1 {
        return Err("ELF file is not little-endian (data must be 1)".into());
    }
    if raw[6] != 1 {
        return Err(
            "ELF file version is not current (version must be 1)".into()
        );
    }
    if raw[7] != 0 {
        return Err("ELF file OS/ABI is not System V (must be 0)".into());
    }

    // Decode ELF header
    let header = ElfHeader::decode(&raw[0..52])?;

    // Validate executable RISC-V file
    if header.e_type != 2 {
        return Err(format!(
            "ELF file is not executable (type={}, expected 2)",
            header.e_type
        )
        .into());
    }
    if header.e_machine != 0xf3 {
        return Err(format!(
            "ELF file is not RISC-V (machine={:#x}, expected 0xf3)",
            header.e_machine
        )
        .into());
    }
    if header.e_version != 1 {
        return Err(format!(
            "ELF file version is not 1 (got {})",
            header.e_version
        )
        .into());
    }

    // Validate header size and entry sizes
    if header.e_ehsize != 52 {
        return Err(format!(
            "unexpected ELF header size: {} (expected 52)",
            header.e_ehsize
        )
        .into());
    }
    if header.e_phentsize != 32 {
        return Err(format!(
            "unexpected program header entry size: {} (expected 32)",
            header.e_phentsize
        )
        .into());
    }
    if header.e_shentsize != 40 {
        return Err(format!(
            "unexpected section header entry size: {} (expected 40)",
            header.e_shentsize
        )
        .into());
    }
    if header.e_phnum < 1 {
        return Err("ELF file has no program headers".into());
    }

    // Load program segments (PT_LOAD only)
    let mut chunks: Vec<(u32, Vec<u8>)> = Vec::new();
    for i in 0..header.e_phnum as usize {
        let offset =
            header.e_phoff as usize + (i * header.e_phentsize as usize);
        if offset + header.e_phentsize as usize > raw.len() {
            return Err(format!(
                "program header {} out of bounds: offset {} size {}",
                i, offset, header.e_phentsize
            )
            .into());
        }

        let ph_data = &raw[offset..offset + header.e_phentsize as usize];
        let ph = ElfProgramHeader::decode(ph_data)?;

        // Only load PT_LOAD segments
        if ph.p_type != PT_LOAD {
            continue;
        }

        // Validate segment file content is within ELF
        let seg_end = ph.p_offset as usize + ph.p_filesz as usize;
        if seg_end > raw.len() {
            return Err(format!(
                "program segment {} extends beyond ELF file (offset {} + size {} > {})",
                i, ph.p_offset, ph.p_filesz, raw.len()
            )
            .into());
        }

        let segment_data = raw[ph.p_offset as usize..seg_end].to_vec();
        chunks.push((ph.p_vaddr, segment_data));
    }

    // Load section header string table
    let shstrtab = load_section_header_string_table(&raw, &header)?;

    // Load section header entries and build segments
    let mut segments = Vec::new();
    let mut strtab: Option<Vec<u8>> = None;
    let mut symtab: Option<Vec<u8>> = None;

    for i in 0..header.e_shnum as usize {
        let offset =
            header.e_shoff as usize + (i * header.e_shentsize as usize);
        if offset + header.e_shentsize as usize > raw.len() {
            return Err(format!(
                "section header {} out of bounds: offset {} size {}",
                i, offset, header.e_shentsize
            )
            .into());
        }

        let sh_data = &raw[offset..offset + header.e_shentsize as usize];
        let sh = ElfSectionHeader::decode(sh_data)?;

        // Get section name
        let section_name = shstrtab.get_string(sh.sh_name as usize).ok();

        // Check for unsupported section types
        if is_unsupported_section_type(sh.sh_type) {
            return Err(format!(
                "ELF file contains unsupported section type: {:#x}",
                sh.sh_type
            )
            .into());
        }

        // Load allocatable sections (PROGBITS or NOBITS with SHF_ALLOC)
        if (sh.sh_type == 1 || sh.sh_type == 8) && (sh.sh_flags & 0x2) != 0 {
            // Find initialization data from program segments
            let mut init = Vec::new();
            for (p_vaddr, seg_data) in &chunks {
                if *p_vaddr <= sh.sh_addr
                    && sh.sh_addr < p_vaddr + seg_data.len() as u32
                {
                    let start_idx = (sh.sh_addr - p_vaddr) as usize;
                    let end_idx =
                        (start_idx + sh.sh_size as usize).min(seg_data.len());
                    init = seg_data[start_idx..end_idx].to_vec();
                    break;
                }
            }

            segments.push(Segment::new(
                sh.sh_addr,
                sh.sh_addr + sh.sh_size,
                (sh.sh_flags & 0x1) != 0, // writable
                (sh.sh_flags & 0x4) != 0, // executable
                init,
            ));
        }
        // Load string table
        else if matches!(section_name.as_deref(), Some(".strtab"))
            && sh.sh_type == SHT_STRTAB
        {
            if sh.sh_offset as usize + sh.sh_size as usize > raw.len() {
                return Err(format!(
                    ".strtab section extends beyond ELF file: offset {} + size {} > {}",
                    sh.sh_offset, sh.sh_size, raw.len()
                )
                .into());
            }
            strtab = Some(
                raw[sh.sh_offset as usize
                    ..(sh.sh_offset as usize + sh.sh_size as usize)]
                    .to_vec(),
            );
        }
        // Load symbol table
        else if matches!(section_name.as_deref(), Some(".symtab"))
            && sh.sh_type == SHT_SYMTAB
        {
            if sh.sh_offset as usize + sh.sh_size as usize > raw.len() {
                return Err(format!(
                    ".symtab section extends beyond ELF file: offset {} + size {} > {}",
                    sh.sh_offset, sh.sh_size, raw.len()
                )
                .into());
            }
            symtab = Some(
                raw[sh.sh_offset as usize
                    ..(sh.sh_offset as usize + sh.sh_size as usize)]
                    .to_vec(),
            );
        }
    }

    let strtab = strtab.ok_or_else(|| {
        "ELF file does not contain .strtab section".to_string()
    })?;
    let symtab = symtab.ok_or_else(|| {
        "ELF file does not contain .symtab section".to_string()
    })?;

    // Parse symbol table
    let (address_symbols, other_symbols, global_pointer) =
        parse_symbol_table(&strtab, &symtab)?;

    // Create machine
    Ok(Machine::new(
        segments,
        header.e_entry,
        global_pointer,
        address_symbols,
        other_symbols,
    ))
}

/// Load the section header string table
fn load_section_header_string_table(
    raw: &[u8],
    header: &ElfHeader,
) -> Result<StringTable> {
    let shstrndx = header.e_shstrndx as usize;
    if shstrndx == 0 {
        // No section header string table
        return Ok(StringTable::new());
    }

    let offset =
        header.e_shoff as usize + (shstrndx * header.e_shentsize as usize);
    if offset + header.e_shentsize as usize > raw.len() {
        return Err(format!(
            "section header string table entry out of bounds: offset {} size {}",
            offset, header.e_shentsize
        )
        .into());
    }

    let sh_data = &raw[offset..offset + header.e_shentsize as usize];
    let sh = ElfSectionHeader::decode(sh_data)?;

    if sh.sh_offset as usize + sh.sh_size as usize > raw.len() {
        return Err(format!(
            "section header string table out of bounds: offset {} + size {} > {}",
            sh.sh_offset, sh.sh_size, raw.len()
        )
        .into());
    }

    let strtab_data = &raw
        [sh.sh_offset as usize..(sh.sh_offset as usize + sh.sh_size as usize)];
    let mut strtab = StringTable::new();

    // Rebuild string table from raw data
    let mut offset = 0;
    while offset < strtab_data.len() {
        let mut end = offset;
        while end < strtab_data.len() && strtab_data[end] != 0 {
            end += 1;
        }

        if offset < end {
            let s =
                String::from_utf8_lossy(&strtab_data[offset..end]).into_owned();
            strtab.add(&s);
        }

        offset = end + 1;
    }

    Ok(strtab)
}

/// Check if a section type is unsupported
fn is_unsupported_section_type(sh_type: u32) -> bool {
    matches!(sh_type, 0x4 | 0x5 | 0x6 | 0x9 | 0xb | 0xe | 0xf | 0x10 | 0x11)
}

/// Type alias for symbol table data: (address_symbols, other_symbols, global_pointer)
type SymbolTableData = (HashMap<u32, String>, HashMap<String, u32>, u32);

/// Parse the symbol table and return symbol maps
fn parse_symbol_table(strtab: &[u8], symtab: &[u8]) -> Result<SymbolTableData> {
    let mut address_symbols: HashMap<u32, String> = HashMap::new();
    let mut other_symbols: HashMap<String, u32> = HashMap::new();
    let mut global_pointer: u32 = 0;

    for i in (0..symtab.len()).step_by(SYMBOL_ENTRY_SIZE) {
        if i + SYMBOL_ENTRY_SIZE > symtab.len() {
            return Err(format!(
                "symbol table entry {} out of bounds: offset {} size {}",
                i / SYMBOL_ENTRY_SIZE,
                i,
                SYMBOL_ENTRY_SIZE
            )
            .into());
        }

        let sym_data = &symtab[i..i + SYMBOL_ENTRY_SIZE];
        let sym = ElfSymbol::decode(sym_data)?;

        // Extract symbol name from string table
        let name = get_symbol_name(strtab, sym.st_name as usize)?;

        // Skip empty names and FILE symbols
        if name.is_empty() || sym.st_info == STT_FILE {
            continue;
        }

        // Track global pointer
        if name == "__global_pointer$" {
            global_pointer = sym.st_value;
            address_symbols.insert(sym.st_value, name);
            continue;
        }

        // Skip internal symbols
        if name.starts_with('$') || name.starts_with("__") {
            continue;
        }

        // Categorize symbol
        if sym.st_shndx > 0 {
            address_symbols.insert(sym.st_value, name);
        } else {
            other_symbols.insert(name, sym.st_value);
        }
    }

    Ok((address_symbols, other_symbols, global_pointer))
}

/// Extract a null-terminated string from the string table
fn get_symbol_name(strtab: &[u8], offset: usize) -> Result<String> {
    if offset >= strtab.len() {
        return Err(format!(
            "symbol name offset {} out of bounds (table size: {})",
            offset,
            strtab.len()
        )
        .into());
    }

    let mut end = offset;
    while end < strtab.len() && strtab[end] != 0 {
        end += 1;
    }

    if end >= strtab.len() {
        return Err("unterminated symbol name in string table".into());
    }

    Ok(String::from_utf8_lossy(&strtab[offset..end]).into_owned())
}
