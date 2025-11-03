use crate::io_abstraction::{IoProvider, SystemIo};
use crate::{Machine, memory::Segment};
use std::collections::HashMap;

pub fn load_elf(filename: &str) -> Result<Machine, String> {
    let raw = std::fs::read(filename)
        .map_err(|e| format!("loading {}: {}", filename, e))?;
    load_elf_from_bytes(&raw, Box::new(SystemIo))
}

pub fn load_elf_from_memory(raw: &[u8]) -> Result<Machine, String> {
    load_elf_from_bytes(raw, Box::new(SystemIo))
}

pub fn load_elf_from_bytes(
    raw: &[u8],
    io_provider: Box<dyn IoProvider>,
) -> Result<Machine, String> {
    if raw.len() < 0x34 {
        return Err("ELF data is too short".to_string());
    }
    if raw[0..4] != *b"\x7fELF" {
        return Err("ELF data does not have ELF magic number".to_string());
    }
    if raw[4] != 1 || raw[5] != 1 || raw[6] != 1 || raw[7] != 0 {
        return Err("ELF data is not a 32-bit, little-endian, version 1, System V ABI ELF file".to_string());
    }

    if u16::from_le_bytes(raw[0x10..0x12].try_into().unwrap()) != 2
        || u16::from_le_bytes(raw[0x12..0x14].try_into().unwrap()) != 0xf3
        || u32::from_le_bytes(raw[0x14..0x18].try_into().unwrap()) != 1
    {
        return Err(
            "ELF data is not an executable, RISC-V, ELF version 1 file"
                .to_string(),
        );
    }

    // 32-bit ELF header offsets
    let e_entry = u32::from_le_bytes(raw[0x18..0x1c].try_into().unwrap());
    let e_phoff =
        u32::from_le_bytes(raw[0x1c..0x20].try_into().unwrap()) as usize;
    let e_shoff =
        u32::from_le_bytes(raw[0x20..0x24].try_into().unwrap()) as usize;

    //let e_flags = u32::from_le_bytes(raw[0x24..0x28].try_into().unwrap());
    let e_ehsize = u16::from_le_bytes(raw[0x28..0x2a].try_into().unwrap());
    let e_phentsize =
        u16::from_le_bytes(raw[0x2a..0x2c].try_into().unwrap()) as usize;
    let e_phnum =
        u16::from_le_bytes(raw[0x2c..0x2e].try_into().unwrap()) as usize;
    let e_shentsize =
        u16::from_le_bytes(raw[0x2e..0x30].try_into().unwrap()) as usize;
    let e_shnum =
        u16::from_le_bytes(raw[0x30..0x32].try_into().unwrap()) as usize;
    let e_shstrndx =
        u16::from_le_bytes(raw[0x32..0x34].try_into().unwrap()) as usize;

    if e_phoff == 0 || e_ehsize != 0x34 || e_phentsize != 0x20 || e_phnum < 1 {
        return Err("ELF data has unexpected RV32 header sizes".to_string());
    }

    // get the loadable segments
    let mut chunks: Vec<(u32, Vec<u8>)> = Vec::new();
    for i in 0..e_phnum {
        // unpack the program header
        let start = e_phoff + e_phentsize * i;
        if start + e_phentsize > raw.len() {
            return Err(
                "ELF data program header entry out of range".to_string()
            );
        }
        // 32-bit program header
        let header = &raw[start..start + e_phentsize];
        let p_type = u32::from_le_bytes(header[0x00..0x04].try_into().unwrap());
        let p_offset =
            u32::from_le_bytes(header[0x04..0x08].try_into().unwrap());
        let p_vaddr =
            u32::from_le_bytes(header[0x08..0x0c].try_into().unwrap());
        //let p_paddr = u32::from_le_bytes(header[0x0c..0x10].try_into().unwrap());
        let p_filesz =
            u32::from_le_bytes(header[0x10..0x14].try_into().unwrap());
        //let p_memsz = u32::from_le_bytes(header[0x14..0x18].try_into().unwrap());
        //let p_align = u32::from_le_bytes(header[0x18..0x1c].try_into().unwrap());

        if p_type != 1 {
            continue;
        }
        if (p_offset as usize + p_filesz as usize) > raw.len() {
            return Err("ELF data program segment out of range".to_string());
        }
        let chunk = (
            p_vaddr,
            raw[p_offset as usize..(p_offset as usize + p_filesz as usize)]
                .to_vec(),
        );
        chunks.push(chunk);
    }

    // get the section header strings
    let start = e_shoff + e_shentsize * e_shstrndx;
    if start + e_shentsize > raw.len() {
        return Err("ELF data section header string table entry out of range"
            .to_string());
    }
    // 32-bit section header offsets
    let header = &raw[start..start + e_shentsize];
    //let sh_name = u32::from_le_bytes(header[0x00..0x04].try_into().unwrap());
    //let sh_type = u32::from_le_bytes(header[0x04..0x08].try_into().unwrap());
    //let sh_flags = u32::from_le_bytes(header[0x08..0x0c].try_into().unwrap());
    //let sh_addr = u32::from_le_bytes(header[0x0c..0x10].try_into().unwrap());
    let sh_offset =
        u32::from_le_bytes(header[0x10..0x14].try_into().unwrap()) as usize;
    let sh_size =
        u32::from_le_bytes(header[0x14..0x18].try_into().unwrap()) as usize;
    //let sh_link = u32::from_le_bytes(header[0x18..0x1c].try_into().unwrap());
    //let sh_info = u32::from_le_bytes(header[0x1c..0x20].try_into().unwrap());
    //let sh_addralign = u32::from_le_bytes(header[0x20..0x24].try_into().unwrap());
    //let sh_entsize = u32::from_le_bytes(header[0x24..0x28].try_into().unwrap());

    if sh_offset + sh_size > raw.len() {
        return Err(
            "ELF data section header string table out of range".to_string()
        );
    }

    // unpack the strings, keyed by offset
    let mut sh_strs = HashMap::new();
    let sh_str_raw = &raw[sh_offset..sh_offset + sh_size];
    let mut start = 0;
    for (i, &b) in sh_str_raw.iter().enumerate() {
        if b == 0 {
            sh_strs.insert(
                start,
                String::from_utf8_lossy(&sh_str_raw[start..i]).into_owned(),
            );
            start = i + 1;
        }
    }

    // read the section headers
    let (mut strs_raw, mut syms_raw) = (Vec::new(), Vec::new());
    let mut segments = Vec::new();

    for i in 0..e_shnum {
        let start = e_shoff + e_shentsize * i;
        if start + e_shentsize > raw.len() {
            return Err("ELF data section header out of range".to_string());
        }

        // 32-bit section header unpacking
        let header = &raw[start..start + e_shentsize];
        let sh_name =
            u32::from_le_bytes(header[0x00..0x04].try_into().unwrap()) as usize;
        let sh_type =
            u32::from_le_bytes(header[0x04..0x08].try_into().unwrap());
        let sh_flags =
            u32::from_le_bytes(header[0x08..0x0c].try_into().unwrap());
        let sh_addr =
            u32::from_le_bytes(header[0x0c..0x10].try_into().unwrap());
        let sh_offset =
            u32::from_le_bytes(header[0x10..0x14].try_into().unwrap()) as usize;
        let sh_size =
            u32::from_le_bytes(header[0x14..0x18].try_into().unwrap()) as usize;
        //let sh_link = u32::from_le_bytes(header[0x18..0x1c].try_into().unwrap());
        //let sh_info = u32::from_le_bytes(header[0x1c..0x20].try_into().unwrap());
        //let sh_addralign = u32::from_le_bytes(header[0x20..0x24].try_into().unwrap());
        //let sh_entsize = u32::from_le_bytes(header[0x24..0x28].try_into().unwrap());

        // check for unsupported features
        if sh_type == 0x4
            || sh_type == 0x5
            || sh_type == 0x6
            || sh_type == 0x9
            || sh_type == 0xb
            || sh_type == 0xe
            || sh_type == 0xf
            || sh_type == 0x10
            || sh_type == 0x11
        {
            return Err(
                "ELF data contains unsupported section type".to_string()
            );
        }

        if (sh_type == 1 || sh_type == 8) && (sh_flags & 0x2) != 0 {
            // in-memory section; see if we have loadable data
            let mut init = Vec::new();
            for &(p_vaddr, ref seg_raw) in &chunks {
                if p_vaddr <= sh_addr
                    && sh_addr < p_vaddr + seg_raw.len() as u32
                {
                    let start_idx = (sh_addr - p_vaddr) as usize;
                    let end_idx = start_idx + sh_size;
                    init = seg_raw[start_idx..end_idx].to_vec();
                }
            }
            segments.push(Segment::new(
                sh_addr,
                sh_addr + sh_size as u32,
                (sh_flags & 0x1) != 0,
                (sh_flags & 0x4) != 0,
                init,
            ));
        } else if sh_strs.get(&sh_name) == Some(&String::from(".strtab"))
            && sh_type == 3
        {
            if sh_offset + sh_size > raw.len() {
                return Err("ELF data string table out of range".to_string());
            }
            strs_raw = raw[sh_offset..sh_offset + sh_size].to_vec();
        } else if sh_strs.get(&sh_name) == Some(&String::from(".symtab"))
            && sh_type == 2
        {
            if sh_offset + sh_size > raw.len() {
                return Err("ELF data symbol table out of range".to_string());
            }
            syms_raw = raw[sh_offset..sh_offset + sh_size].to_vec();
        }
    }

    if strs_raw.is_empty() {
        return Err("ELF data: no string table found".to_string());
    }
    if syms_raw.is_empty() {
        return Err("ELF data: no symbol table found".to_string());
    }

    // parse the symbol table
    let mut address_symbols: HashMap<u32, String> = HashMap::new();
    let mut other_symbols: HashMap<String, u32> = HashMap::new();
    let mut global_pointer: u32 = 0;
    // 32-bit ELF symbol table entries are 16 bytes
    const SYMBOL_SIZE: usize = 16;

    for start in (0..syms_raw.len()).step_by(SYMBOL_SIZE) {
        if start + SYMBOL_SIZE > syms_raw.len() {
            return Err("ELF data symbol table entry out of range".to_string());
        }
        // 32-bit symbol entry format:
        let symbol = &syms_raw[start..start + SYMBOL_SIZE];
        let st_name =
            u32::from_le_bytes(symbol[0x00..0x04].try_into().unwrap()) as usize;
        let st_value =
            u32::from_le_bytes(symbol[0x04..0x08].try_into().unwrap());
        let _st_size =
            u32::from_le_bytes(symbol[0x08..0x0c].try_into().unwrap());
        let st_info = symbol[0x0c];
        //let st_other = symbol[0x0d];
        let st_shndx =
            u16::from_le_bytes(symbol[0x0e..0x10].try_into().unwrap());

        let mut end = st_name;
        while end < strs_raw.len() && strs_raw[end] != 0 {
            end += 1;
        }
        if end >= strs_raw.len() {
            return Err("ELF data symbol name out of range".to_string());
        }
        let name =
            String::from_utf8_lossy(&strs_raw[st_name..end]).into_owned();

        if name.is_empty() || st_info == 4 {
            // skip section entries and object file names
            continue;
        } else if name == "__global_pointer$" {
            // keep global pointer
            global_pointer = st_value;
            address_symbols.insert(st_value, name);
            continue;
        } else if name.starts_with('$') || name.starts_with("__") {
            // skip internal names
            continue;
        }

        // sort into text, data/bss, and other symbols
        if st_shndx > 0 {
            address_symbols.insert(st_value, name);
        } else {
            other_symbols.insert(name, st_value);
        }
    }

    // allocate address space
    Ok(Machine::with_io_provider(
        segments,
        e_entry,
        global_pointer,
        address_symbols,
        other_symbols,
        io_provider,
    ))
}
