// ELF binary format generation for RISC-V 64-bit little-endian executables
//
// This module provides data structures and encoding functions to generate
// executable ELF binaries matching the layout produced by GNU as + ld.
//
// Reference: ELF-64 Object File Format, Version 1.5 Draft 2
// https://refspecs.linuxfoundation.org/elf/elf.pdf

use crate::ast::{LineContent, Segment, Source};
use std::collections::HashMap;

// ============================================================================
// ELF Constants
// ============================================================================

// ELF Identification
const EI_MAG0: u8 = 0x7f;
const EI_MAG1: u8 = b'E';
const EI_MAG2: u8 = b'L';
const EI_MAG3: u8 = b'F';
const EI_CLASS: u8 = 2; // ELFCLASS64
const EI_DATA: u8 = 1; // ELFDATA2LSB (little endian)
const EI_VERSION: u8 = 1; // EV_CURRENT
const EI_OSABI: u8 = 0; // ELFOSABI_SYSV
const EI_ABIVERSION: u8 = 0;

// ELF File Types
const ET_EXEC: u16 = 2; // Executable file

// Machine Type
const EM_RISCV: u16 = 0xF3; // RISC-V

// Object File Version
const EV_CURRENT: u32 = 1;

// ELF Header Flags (for RISC-V)
const EF_RISCV_FLOAT_ABI_DOUBLE: u32 = 0x4; // Double-precision FP ABI

// Section Types
const SHT_NULL: u32 = 0;
const SHT_PROGBITS: u32 = 1;
const SHT_SYMTAB: u32 = 2;
const SHT_STRTAB: u32 = 3;
const SHT_NOBITS: u32 = 8;
const SHT_RISCV_ATTRIBUTES: u32 = 0x7000_0003;

// Section Flags
const SHF_WRITE: u64 = 0x1;
const SHF_ALLOC: u64 = 0x2;
const SHF_EXECINSTR: u64 = 0x4;

// Program Header Types
const PT_LOAD: u32 = 1;
const PT_RISCV_ATTRIBUTES: u32 = 0x7000_0003;

// Program Header Flags
const PF_X: u32 = 0x1; // Execute
const PF_W: u32 = 0x2; // Write
const PF_R: u32 = 0x4; // Read

// Symbol Binding
const STB_LOCAL: u8 = 0;
const STB_GLOBAL: u8 = 1;

// Symbol Types
const STT_NOTYPE: u8 = 0;
const STT_SECTION: u8 = 3;
const STT_FILE: u8 = 4;

// Special Section Indices
const SHN_UNDEF: u16 = 0;
const SHN_ABS: u16 = 0xfff1;

// ============================================================================
// ELF Data Structures
// ============================================================================

/// ELF-64 File Header
#[derive(Debug, Clone)]
pub struct Elf64Header {
    pub e_ident: [u8; 16], // ELF identification
    pub e_type: u16,       // Object file type
    pub e_machine: u16,    // Machine type
    pub e_version: u32,    // Object file version
    pub e_entry: u64,      // Entry point address
    pub e_phoff: u64,      // Program header offset
    pub e_shoff: u64,      // Section header offset
    pub e_flags: u32,      // Processor-specific flags
    pub e_ehsize: u16,     // ELF header size
    pub e_phentsize: u16,  // Program header entry size
    pub e_phnum: u16,      // Number of program headers
    pub e_shentsize: u16,  // Section header entry size
    pub e_shnum: u16,      // Number of section headers
    pub e_shstrndx: u16,   // Section name string table index
}

impl Default for Elf64Header {
    fn default() -> Self {
        Self::new()
    }
}

impl Elf64Header {
    pub fn new() -> Self {
        let mut e_ident = [0u8; 16];
        e_ident[0] = EI_MAG0;
        e_ident[1] = EI_MAG1;
        e_ident[2] = EI_MAG2;
        e_ident[3] = EI_MAG3;
        e_ident[4] = EI_CLASS;
        e_ident[5] = EI_DATA;
        e_ident[6] = EI_VERSION;
        e_ident[7] = EI_OSABI;
        e_ident[8] = EI_ABIVERSION;
        // Bytes 9-15 are padding (already zeroed)

        Self {
            e_ident,
            e_type: ET_EXEC,
            e_machine: EM_RISCV,
            e_version: EV_CURRENT,
            e_entry: 0,
            e_phoff: 64, // Program headers start right after ELF header
            e_shoff: 0,  // Will be set later
            e_flags: EF_RISCV_FLOAT_ABI_DOUBLE,
            e_ehsize: 64,
            e_phentsize: 56,
            e_phnum: 0, // Will be set later
            e_shentsize: 64,
            e_shnum: 0,    // Will be set later
            e_shstrndx: 0, // Will be set later
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(64);
        bytes.extend_from_slice(&self.e_ident);
        bytes.extend_from_slice(&self.e_type.to_le_bytes());
        bytes.extend_from_slice(&self.e_machine.to_le_bytes());
        bytes.extend_from_slice(&self.e_version.to_le_bytes());
        bytes.extend_from_slice(&self.e_entry.to_le_bytes());
        bytes.extend_from_slice(&self.e_phoff.to_le_bytes());
        bytes.extend_from_slice(&self.e_shoff.to_le_bytes());
        bytes.extend_from_slice(&self.e_flags.to_le_bytes());
        bytes.extend_from_slice(&self.e_ehsize.to_le_bytes());
        bytes.extend_from_slice(&self.e_phentsize.to_le_bytes());
        bytes.extend_from_slice(&self.e_phnum.to_le_bytes());
        bytes.extend_from_slice(&self.e_shentsize.to_le_bytes());
        bytes.extend_from_slice(&self.e_shnum.to_le_bytes());
        bytes.extend_from_slice(&self.e_shstrndx.to_le_bytes());
        bytes
    }
}

/// ELF-64 Program Header
#[derive(Debug, Clone)]
pub struct Elf64ProgramHeader {
    pub p_type: u32,   // Segment type
    pub p_flags: u32,  // Segment flags
    pub p_offset: u64, // Segment file offset
    pub p_vaddr: u64,  // Segment virtual address
    pub p_paddr: u64,  // Segment physical address
    pub p_filesz: u64, // Segment size in file
    pub p_memsz: u64,  // Segment size in memory
    pub p_align: u64,  // Segment alignment
}

impl Elf64ProgramHeader {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(56);
        bytes.extend_from_slice(&self.p_type.to_le_bytes());
        bytes.extend_from_slice(&self.p_flags.to_le_bytes());
        bytes.extend_from_slice(&self.p_offset.to_le_bytes());
        bytes.extend_from_slice(&self.p_vaddr.to_le_bytes());
        bytes.extend_from_slice(&self.p_paddr.to_le_bytes());
        bytes.extend_from_slice(&self.p_filesz.to_le_bytes());
        bytes.extend_from_slice(&self.p_memsz.to_le_bytes());
        bytes.extend_from_slice(&self.p_align.to_le_bytes());
        bytes
    }
}

/// Computes the expected combined size of the ELF header and program headers.
///
/// # Arguments
/// * `num_segments` - The number of program segments (e.g., 1 for text-only, 2 for text + data/bss).
///
/// # Returns
/// The total size in bytes (ELF header + program headers).
pub fn compute_header_size(num_segments: i64) -> i64 {
    const ELF_HEADER_SIZE: i64 = 64;  // From e_ehsize in Elf64Header
    const PROGRAM_HEADER_SIZE: i64 = 56;  // From e_phentsize in Elf64Header

    ELF_HEADER_SIZE + (num_segments * PROGRAM_HEADER_SIZE)
}


/// ELF-64 Section Header
#[derive(Debug, Clone)]
pub struct Elf64SectionHeader {
    pub sh_name: u32,      // Section name (string table index)
    pub sh_type: u32,      // Section type
    pub sh_flags: u64,     // Section flags
    pub sh_addr: u64,      // Section virtual address
    pub sh_offset: u64,    // Section file offset
    pub sh_size: u64,      // Section size in bytes
    pub sh_link: u32,      // Link to another section
    pub sh_info: u32,      // Additional section information
    pub sh_addralign: u64, // Section alignment
    pub sh_entsize: u64,   // Entry size if section holds table
}

impl Elf64SectionHeader {
    pub fn null() -> Self {
        Self {
            sh_name: 0,
            sh_type: SHT_NULL,
            sh_flags: 0,
            sh_addr: 0,
            sh_offset: 0,
            sh_size: 0,
            sh_link: 0,
            sh_info: 0,
            sh_addralign: 0,
            sh_entsize: 0,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(64);
        bytes.extend_from_slice(&self.sh_name.to_le_bytes());
        bytes.extend_from_slice(&self.sh_type.to_le_bytes());
        bytes.extend_from_slice(&self.sh_flags.to_le_bytes());
        bytes.extend_from_slice(&self.sh_addr.to_le_bytes());
        bytes.extend_from_slice(&self.sh_offset.to_le_bytes());
        bytes.extend_from_slice(&self.sh_size.to_le_bytes());
        bytes.extend_from_slice(&self.sh_link.to_le_bytes());
        bytes.extend_from_slice(&self.sh_info.to_le_bytes());
        bytes.extend_from_slice(&self.sh_addralign.to_le_bytes());
        bytes.extend_from_slice(&self.sh_entsize.to_le_bytes());
        bytes
    }
}

/// ELF-64 Symbol Table Entry
#[derive(Debug, Clone)]
pub struct Elf64Symbol {
    pub st_name: u32,  // Symbol name (string table index)
    pub st_info: u8,   // Symbol type and binding
    pub st_other: u8,  // Symbol visibility
    pub st_shndx: u16, // Section index
    pub st_value: u64, // Symbol value
    pub st_size: u64,  // Symbol size
}

impl Elf64Symbol {
    /// Create undefined symbol (entry 0)
    pub fn null() -> Self {
        Self {
            st_name: 0,
            st_info: 0,
            st_other: 0,
            st_shndx: SHN_UNDEF,
            st_value: 0,
            st_size: 0,
        }
    }

    /// Create section symbol
    pub fn section(section_index: u16) -> Self {
        Self {
            st_name: 0,
            st_info: make_st_info(STB_LOCAL, STT_SECTION),
            st_other: 0,
            st_shndx: section_index,
            st_value: 0,
            st_size: 0,
        }
    }

    /// Create FILE symbol
    pub fn file(name_index: u32) -> Self {
        Self {
            st_name: name_index,
            st_info: make_st_info(STB_LOCAL, STT_FILE),
            st_other: 0,
            st_shndx: SHN_ABS,
            st_value: 0,
            st_size: 0,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(24);
        bytes.extend_from_slice(&self.st_name.to_le_bytes());
        bytes.push(self.st_info);
        bytes.push(self.st_other);
        bytes.extend_from_slice(&self.st_shndx.to_le_bytes());
        bytes.extend_from_slice(&self.st_value.to_le_bytes());
        bytes.extend_from_slice(&self.st_size.to_le_bytes());
        bytes
    }
}

/// Helper to create st_info field from binding and type
fn make_st_info(bind: u8, typ: u8) -> u8 {
    (bind << 4) | (typ & 0xf)
}

// ============================================================================
// String Table Builder
// ============================================================================

/// String table builder that deduplicates strings
pub struct StringTable {
    strings: Vec<u8>,
    offsets: HashMap<String, u32>,
}

impl Default for StringTable {
    fn default() -> Self {
        Self::new()
    }
}

impl StringTable {
    pub fn new() -> Self {
        // String tables start with a null byte
        Self {
            strings: vec![0],
            offsets: HashMap::new(),
        }
    }

    /// Add a string and return its offset
    pub fn add(&mut self, s: &str) -> u32 {
        if let Some(&offset) = self.offsets.get(s) {
            return offset;
        }

        let offset = self.strings.len() as u32;
        self.offsets.insert(s.to_string(), offset);
        self.strings.extend_from_slice(s.as_bytes());
        self.strings.push(0); // Null terminator
        offset
    }

    pub fn data(&self) -> &[u8] {
        &self.strings
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.strings.len() <= 1
    }
}

// ============================================================================
// RISC-V Attributes Section
// ============================================================================

/// Generate .riscv.attributes section content
///
/// This section describes the RISC-V ISA features used by the binary.
/// Format follows the ELF attributes specification with RISC-V extensions.
///
/// For RV64IM (no floating point), we generate:
/// "rv64i2p1_m2p0"
pub fn generate_riscv_attributes() -> Vec<u8> {
    // For simplicity, we'll generate attributes for RV64IM
    // matching the basic ISA string without floating point extensions
    let arch_string = "rv64i2p1_m2p0";

    let mut attrs = Vec::new();

    // Format version (always 'A' = 0x41)
    attrs.push(b'A');

    // Total length of attribute section (will be patched)
    let length_pos = attrs.len();
    attrs.extend_from_slice(&[0u8; 4]);

    // Vendor name (always "riscv" for RISC-V)
    attrs.extend_from_slice(b"riscv\0");

    // File attributes tag (1)
    attrs.push(1);

    // Length of file attributes subsection (will be patched)
    let file_attrs_length_pos = attrs.len();
    attrs.extend_from_slice(&[0u8; 4]);

    // Tag_RISCV_arch (5): RISC-V architecture string
    attrs.push(5);
    attrs.extend_from_slice(arch_string.as_bytes());
    attrs.push(0); // Null terminator

    // Patch file attributes length
    let file_attrs_length = (attrs.len() - file_attrs_length_pos) as u32;
    attrs[file_attrs_length_pos..file_attrs_length_pos + 4]
        .copy_from_slice(&file_attrs_length.to_le_bytes());

    // Patch total length
    let total_length = (attrs.len() - length_pos) as u32;
    attrs[length_pos..length_pos + 4].copy_from_slice(&total_length.to_le_bytes());

    attrs
}

// ============================================================================
// ELF Builder
// ============================================================================

pub struct ElfBuilder {
    pub header: Elf64Header,
    pub program_headers: Vec<Elf64ProgramHeader>,
    pub section_headers: Vec<Elf64SectionHeader>,
    pub section_names: StringTable,
    pub symbol_table: Vec<Elf64Symbol>,
    pub symbol_names: StringTable,
    pub text_data: Vec<u8>,
    pub data_data: Vec<u8>,
    pub bss_size: u64,
    pub riscv_attributes: Vec<u8>,
    pub text_start: u64,
    pub data_start: u64,
    pub bss_start: u64,
    estimated_header_size: u64,
}

impl ElfBuilder {
    pub fn new(text_start: u64, estimated_header_size: u64) -> Self {
        Self {
            header: Elf64Header::new(),
            program_headers: Vec::new(),
            section_headers: Vec::new(),
            section_names: StringTable::new(),
            symbol_table: Vec::new(),
            symbol_names: StringTable::new(),
            text_data: Vec::new(),
            data_data: Vec::new(),
            bss_size: 0,
            riscv_attributes: generate_riscv_attributes(),
            text_start,
            data_start: 0,
            bss_start: 0,
            estimated_header_size,
        }
    }

    /// Set segment data
    pub fn set_segments(
        &mut self,
        text: Vec<u8>,
        data: Vec<u8>,
        bss_size: u64,
        data_start: u64,
        bss_start: u64,
    ) {
        self.text_data = text;
        self.data_data = data;
        self.bss_size = bss_size;
        self.data_start = data_start;
        self.bss_start = bss_start;
    }

    /// Add a symbol to the symbol table
    pub fn add_symbol(&mut self, symbol: Elf64Symbol) {
        self.symbol_table.push(symbol);
    }

    /// Build the complete ELF file
    pub fn build(mut self, entry_point: u64) -> Vec<u8> {
        self.header.e_entry = entry_point;

        let mut output = vec![0; 64];

        // Pre-populate section name string table (needed before building section headers)
        // This ensures all section names are in the string table before we reference them
        self.section_names.add(".text");
        if !self.data_data.is_empty() {
            self.section_names.add(".data");
        }
        if self.bss_size > 0 {
            self.section_names.add(".bss");
        }
        self.section_names.add(".riscv.attributes");
        self.section_names.add(".symtab");
        self.section_names.add(".strtab");
        self.section_names.add(".shstrtab");

        // Reserve space for program headers (will write later after we know offsets)
        self.build_program_headers();
        let phoff = output.len() as u64;
        let ph_size = self.program_headers.len() as u64 * 56;
        let actual_header_size = phoff + ph_size;

        // Verification check: ensure the estimated header size matches the actual size.
        // A mismatch indicates a bug in the program header count estimation.
        assert_eq!(
            self.estimated_header_size, actual_header_size,
            "Mismatch between estimated and actual ELF header size. The number of program headers was likely estimated incorrectly."
        );
        output.resize(output.len() + ph_size as usize, 0);

        // --- Section Layout ---
        let page_size = 0x1000;

        // .text section starts right after the program headers
        let text_offset = output.len() as u64;
        output.extend_from_slice(&self.text_data);

        // .data section is page-aligned in the file to support mmap.
        // Pad the file with zeros to align the data offset.
        let data_offset = if !self.data_data.is_empty() || self.bss_size > 0 {
            let current_len = output.len() as u64;
            let padding = (page_size - (current_len % page_size)) % page_size;
            output.resize(output.len() + padding as usize, 0);
            Some(output.len() as u64)
        } else {
            None
        };

        if let Some(_offset) = data_offset {
            output.extend_from_slice(&self.data_data);
        }

        // .riscv.attributes section (not loaded into memory)
        let riscv_attrs_offset = output.len() as u64;
        output.extend_from_slice(&self.riscv_attributes);

        // Build symbol table
        if self.symbol_table.is_empty() {
            self.symbol_table.push(Elf64Symbol::null());
        }

        // Symbol table section
        let symtab_offset = output.len() as u64;
        for sym in &self.symbol_table {
            output.extend_from_slice(&sym.encode());
        }

        // String table section (.strtab)
        let strtab_offset = output.len() as u64;
        output.extend_from_slice(self.symbol_names.data());

        // Section name string table (.shstrtab)
        let shstrtab_offset = output.len() as u64;
        output.extend_from_slice(self.section_names.data());

        // Build section headers
        self.build_section_headers(
            text_offset,
            data_offset,
            riscv_attrs_offset,
            symtab_offset,
            strtab_offset,
            shstrtab_offset,
        );

        // Write section headers
        let shoff = output.len() as u64;
        for sh in &self.section_headers {
            output.extend_from_slice(&sh.encode());
        }

        // --- Finalize Program Headers ---
        // Now that all offsets and sizes are known, update the program headers.
        let headers_size = phoff + ph_size;
        // self.text_start is the address of the first instruction (e.g., _start),
        // which is located after the file headers. The segment's base vaddr is
        // therefore text_start - headers_size.
        let base_vaddr = self.text_start.saturating_sub(headers_size);

        // Program Header 0: RISCV_ATTRIBUTES
        if let Some(ph) = self.program_headers.get_mut(0) {
            ph.p_offset = riscv_attrs_offset;
        }

        // Program Header 1: LOAD .text
        if let Some(ph) = self.program_headers.get_mut(1) {
            // This segment starts at the base virtual address and includes the
            // ELF and program headers in its memory mapping.
            ph.p_vaddr = base_vaddr;
            ph.p_paddr = base_vaddr;
            ph.p_offset = 0;
            ph.p_filesz = text_offset + self.text_data.len() as u64;
            ph.p_memsz = ph.p_filesz;
        }

        // Program Header 2: LOAD .data/.bss
        #[allow(clippy::collapsible_if)]
        if self.program_headers.len() > 2 {
            if let Some(ph) = self.program_headers.get_mut(2) {
                ph.p_offset = data_offset.unwrap_or(0);
                // The vaddr for data is already correctly calculated in expressions.rs
                // and set during the initial program header creation.
            }
        }

        // Update ELF header
        self.header.e_phoff = phoff;
        self.header.e_phnum = self.program_headers.len() as u16;
        self.header.e_shoff = shoff;
        self.header.e_shnum = self.section_headers.len() as u16;
        self.header.e_shstrndx = (self.section_headers.len() - 1) as u16; // .shstrtab is last

        // Write ELF header at the beginning
        output[0..64].copy_from_slice(&self.header.encode());

        // Write program headers at their reserved location
        let mut ph_bytes = Vec::new();
        for ph in &self.program_headers {
            ph_bytes.extend_from_slice(&ph.encode());
        }
        output[phoff as usize..(phoff as usize + ph_bytes.len())].copy_from_slice(&ph_bytes);

        output
    }

    fn build_program_headers(&mut self) {
        // RISCV_ATTRIBUTES segment (non-allocating)
        self.program_headers.push(Elf64ProgramHeader {
            p_type: PT_RISCV_ATTRIBUTES,
            p_flags: PF_R,
            p_offset: 0, // Will be calculated during build
            p_vaddr: 0,
            p_paddr: 0,
            p_filesz: self.riscv_attributes.len() as u64,
            p_memsz: 0,
            p_align: 1,
        });

        // LOAD segment for .text
        let text_filesz = self.text_data.len() as u64;
        self.program_headers.push(Elf64ProgramHeader {
            p_type: PT_LOAD,
            p_flags: PF_R | PF_X,
            p_offset: 0, // Will be set during build (0x1000 page aligned)
            p_vaddr: self.text_start,
            p_paddr: self.text_start,
            p_filesz: text_filesz,
            p_memsz: text_filesz,
            p_align: 0x1000,
        });

        // LOAD segment for .data + .bss (if present)
        if !self.data_data.is_empty() || self.bss_size > 0 {
            let data_filesz = self.data_data.len() as u64;
            let data_memsz = data_filesz + self.bss_size;

            self.program_headers.push(Elf64ProgramHeader {
                p_type: PT_LOAD,
                p_flags: PF_R | PF_W,
                p_offset: 0, // Will be set during build
                p_vaddr: self.data_start,
                p_paddr: self.data_start,
                p_filesz: data_filesz,
                p_memsz: data_memsz,
                p_align: 0x1000,
            });
        }
    }

    fn build_section_headers(
        &mut self,
        text_offset: u64,
        data_offset: Option<u64>,
        riscv_attrs_offset: u64,
        symtab_offset: u64,
        strtab_offset: u64,
        shstrtab_offset: u64,
    ) {
        let mut section_index = 0u16;

        // Section 0: NULL
        self.section_headers.push(Elf64SectionHeader::null());
        section_index += 1;

        // Section 1: .text
        self.section_headers.push(Elf64SectionHeader {
            sh_name: self.section_names.add(".text"),
            sh_type: SHT_PROGBITS,
            sh_flags: SHF_ALLOC | SHF_EXECINSTR,
            sh_addr: self.text_start,
            sh_offset: text_offset,
            sh_size: self.text_data.len() as u64,
            sh_link: 0,
            sh_info: 0,
            sh_addralign: 4,
            sh_entsize: 0,
        });
        section_index += 1;

        // Section 2: .data (if present)
        if !self.data_data.is_empty() {
            self.section_headers.push(Elf64SectionHeader {
                sh_name: self.section_names.add(".data"),
                sh_type: SHT_PROGBITS,
                sh_flags: SHF_WRITE | SHF_ALLOC,
                sh_addr: self.data_start,
                sh_offset: data_offset.unwrap(),
                sh_size: self.data_data.len() as u64,
                sh_link: 0,
                sh_info: 0,
                sh_addralign: 1,
                sh_entsize: 0,
            });
            section_index += 1;
        }

        // Section 3: .bss (if present)
        if self.bss_size > 0 {
            self.section_headers.push(Elf64SectionHeader {
                sh_name: self.section_names.add(".bss"),
                sh_type: SHT_NOBITS,
                sh_flags: SHF_WRITE | SHF_ALLOC,
                sh_addr: self.bss_start,
                sh_offset: data_offset.unwrap_or(text_offset + self.text_data.len() as u64),
                sh_size: self.bss_size,
                sh_link: 0,
                sh_info: 0,
                sh_addralign: 1,
                sh_entsize: 0,
            });
            section_index += 1;
        }

        // Section: .riscv.attributes
        self.section_headers.push(Elf64SectionHeader {
            sh_name: self.section_names.add(".riscv.attributes"),
            sh_type: SHT_RISCV_ATTRIBUTES,
            sh_flags: 0, // Not allocated
            sh_addr: 0,
            sh_offset: riscv_attrs_offset,
            sh_size: self.riscv_attributes.len() as u64,
            sh_link: 0,
            sh_info: 0,
            sh_addralign: 1,
            sh_entsize: 0,
        });
        section_index += 1;

        // Section: .symtab
        let strtab_section_index = section_index + 1; // .strtab comes next
        let first_global = self
            .symbol_table
            .iter()
            .position(|sym| (sym.st_info >> 4) == STB_GLOBAL)
            .unwrap_or(self.symbol_table.len()) as u32;

        self.section_headers.push(Elf64SectionHeader {
            sh_name: self.section_names.add(".symtab"),
            sh_type: SHT_SYMTAB,
            sh_flags: 0,
            sh_addr: 0,
            sh_offset: symtab_offset,
            sh_size: (self.symbol_table.len() * 24) as u64,
            sh_link: strtab_section_index as u32,
            sh_info: first_global, // Index of first global symbol
            sh_addralign: 8,
            sh_entsize: 24,
        });

        // Section: .strtab
        self.section_headers.push(Elf64SectionHeader {
            sh_name: self.section_names.add(".strtab"),
            sh_type: SHT_STRTAB,
            sh_flags: 0,
            sh_addr: 0,
            sh_offset: strtab_offset,
            sh_size: self.symbol_names.len() as u64,
            sh_link: 0,
            sh_info: 0,
            sh_addralign: 1,
            sh_entsize: 0,
        });

        // Section: .shstrtab
        self.section_headers.push(Elf64SectionHeader {
            sh_name: self.section_names.add(".shstrtab"),
            sh_type: SHT_STRTAB,
            sh_flags: 0,
            sh_addr: 0,
            sh_offset: shstrtab_offset,
            sh_size: self.section_names.len() as u64,
            sh_link: 0,
            sh_info: 0,
            sh_addralign: 1,
            sh_entsize: 0,
        });


    }
}

// ============================================================================
// Symbol Table Generation
// ============================================================================

/// Build symbol table from Source AST
///
/// Symbol ordering matches GNU toolchain:
/// 1. Null symbol (entry 0)
/// 2. Section symbols (.text, .data, .bss if present)
/// 3. For each source file:
///    a. FILE symbol
///    b. Special $xrv64i2p1_m2p0 marker symbol
///    c. Local labels from that file
/// 4. Global symbols (including linker-provided symbols)
pub fn build_symbol_table(
    source: &Source,
    builder: &mut ElfBuilder,
    text_start: u64,
    data_start: u64,
    bss_start: u64,
    has_data: bool,
    has_bss: bool,
) {
    // Entry 0: Null symbol
    builder.add_symbol(Elf64Symbol::null());

    // Section symbols
    let text_section_index = 1u16;
    builder.add_symbol(Elf64Symbol::section(text_section_index));

    let mut data_section_index = None;
    if has_data {
        data_section_index = Some(2u16);
        builder.add_symbol(Elf64Symbol::section(2));
    }

    let mut bss_section_index = None;
    if has_bss {
        let idx = if has_data { 3u16 } else { 2u16 };
        bss_section_index = Some(idx);
        builder.add_symbol(Elf64Symbol::section(idx));
    }

    // For each source file, add FILE symbol and local labels
    for (file_index, source_file) in source.files.iter().enumerate() {
        // FILE symbol (basename of source file)
        let file_name = std::path::Path::new(&source_file.file)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&source_file.file);
        let file_name_idx = builder.symbol_names.add(file_name);
        builder.add_symbol(Elf64Symbol::file(file_name_idx));

        // Add special $xrv64i2p1_m2p0 marker symbol
        // This marks the start of code from this file
        let marker_name = builder
            .symbol_names
            .add("$xrv64i2p1_m2p0");

        // Find the first .text line in this file to use as the marker address
        let mut marker_addr = text_start;
        for line in &source_file.lines {
            if line.segment == Segment::Text {
                marker_addr = text_start + line.offset as u64;
                break;
            }
        }

        builder.add_symbol(Elf64Symbol {
            st_name: marker_name,
            st_info: make_st_info(STB_LOCAL, STT_NOTYPE),
            st_other: 0,
            st_shndx: text_section_index,
            st_value: marker_addr,
            st_size: 0,
        });

        // Add local labels (non-global labels and non-.equ symbols)
        for (line_index, line) in source_file.lines.iter().enumerate() {
            if let LineContent::Label(name) = &line.content {
                // Skip if this label is declared global
                let is_global = source.global_symbols.iter().any(|g| {
                    &g.symbol == name
                        && g.definition_pointer.file_index == file_index
                        && g.definition_pointer.line_index == line_index
                });

                // Skip numeric labels (they are local/temporary)
                if name.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }

                if !is_global {
                    let name_idx = builder.symbol_names.add(name);
                    let (addr, section_idx) = match line.segment {
                        Segment::Text => (text_start + line.offset as u64, text_section_index),
                        Segment::Data => {
                            (data_start + line.offset as u64, data_section_index.unwrap())
                        }
                        Segment::Bss => {
                            (bss_start + line.offset as u64, bss_section_index.unwrap())
                        }
                    };

                    builder.add_symbol(Elf64Symbol {
                        st_name: name_idx,
                        st_info: make_st_info(STB_LOCAL, STT_NOTYPE),
                        st_other: 0,
                        st_shndx: section_idx,
                        st_value: addr,
                        st_size: 0,
                    });
                }
            }
        }
    }

    // Add linker-provided symbols (all global)
    // These come before user-defined global symbols

    // __global_pointer$ = data_start + 0x800
    let gp_name = builder.symbol_names.add("__global_pointer$");
    builder.add_symbol(Elf64Symbol {
        st_name: gp_name,
        st_info: make_st_info(STB_GLOBAL, STT_NOTYPE),
        st_other: 0,
        st_shndx: SHN_ABS,
        st_value: data_start + 0x800,
        st_size: 0,
    });

    // __SDATA_BEGIN__ = data_start (if data exists)
    if has_data || has_bss {
        let sdata_begin = builder.symbol_names.add("__SDATA_BEGIN__");
        let section = if has_data {
            data_section_index.unwrap()
        } else {
            bss_section_index.unwrap()
        };
        let addr = if has_data { data_start } else { bss_start };
        builder.add_symbol(Elf64Symbol {
            st_name: sdata_begin,
            st_info: make_st_info(STB_GLOBAL, STT_NOTYPE),
            st_other: 0,
            st_shndx: section,
            st_value: addr,
            st_size: 0,
        });
    }

    // Add user-defined global symbols
    for global in &source.global_symbols {
        let file_index = global.definition_pointer.file_index;
        let line_index = global.definition_pointer.line_index;
        let line = &source.files[file_index].lines[line_index];

        let name_idx = builder.symbol_names.add(&global.symbol);
        let (addr, section_idx) = match line.segment {
            Segment::Text => (text_start + line.offset as u64, text_section_index),
            Segment::Data => (data_start + line.offset as u64, data_section_index.unwrap()),
            Segment::Bss => (bss_start + line.offset as u64, bss_section_index.unwrap()),
        };

        builder.add_symbol(Elf64Symbol {
            st_name: name_idx,
            st_info: make_st_info(STB_GLOBAL, STT_NOTYPE),
            st_other: 0,
            st_shndx: section_idx,
            st_value: addr,
            st_size: 0,
        });
    }

    // More linker-provided symbols (at end)
    let end_text = text_start + source.text_size as u64;
    let end_data = if has_data {
        data_start + source.data_size as u64
    } else if has_bss {
        bss_start
    } else {
        end_text
    };
    let end_bss = if has_bss {
        bss_start + source.bss_size as u64
    } else {
        end_data
    };

    // __bss_start
    let bss_start_name = builder.symbol_names.add("__bss_start");
    let bss_start_section = if has_bss {
        bss_section_index.unwrap()
    } else if has_data {
        data_section_index.unwrap()
    } else {
        text_section_index
    };
    builder.add_symbol(Elf64Symbol {
        st_name: bss_start_name,
        st_info: make_st_info(STB_GLOBAL, STT_NOTYPE),
        st_other: 0,
        st_shndx: bss_start_section,
        st_value: if has_bss { bss_start } else { end_data },
        st_size: 0,
    });

    // __DATA_BEGIN__
    let data_begin_name = builder.symbol_names.add("__DATA_BEGIN__");
    builder.add_symbol(Elf64Symbol {
        st_name: data_begin_name,
        st_info: make_st_info(STB_GLOBAL, STT_NOTYPE),
        st_other: 0,
        st_shndx: bss_start_section,
        st_value: if has_bss { bss_start } else { end_data },
        st_size: 0,
    });

    // __BSS_END__
    let bss_end_name = builder.symbol_names.add("__BSS_END__");
    let bss_end_section = if has_bss {
        bss_section_index.unwrap()
    } else if has_data {
        data_section_index.unwrap()
    } else {
        text_section_index
    };
    builder.add_symbol(Elf64Symbol {
        st_name: bss_end_name,
        st_info: make_st_info(STB_GLOBAL, STT_NOTYPE),
        st_other: 0,
        st_shndx: bss_end_section,
        st_value: end_bss,
        st_size: 0,
    });

    // _edata = end of .data (or end of .bss if no .data)
    let edata_name = builder.symbol_names.add("_edata");
    builder.add_symbol(Elf64Symbol {
        st_name: edata_name,
        st_info: make_st_info(STB_GLOBAL, STT_NOTYPE),
        st_other: 0,
        st_shndx: bss_start_section,
        st_value: if has_bss { bss_start } else { end_data },
        st_size: 0,
    });

    // _end = absolute end of all sections
    let end_name = builder.symbol_names.add("_end");
    builder.add_symbol(Elf64Symbol {
        st_name: end_name,
        st_info: make_st_info(STB_GLOBAL, STT_NOTYPE),
        st_other: 0,
        st_shndx: bss_end_section,
        st_value: end_bss,
        st_size: 0,
    });
}
