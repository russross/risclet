// ELF binary format handling for RISC-V 32-bit little-endian executables
//
// This module provides unified data structures and operations for ELF files,
// used by both the assembler (elf_builder) and simulator (elf_loader).
//
// Reference: ELF-32 Object File Format, Version 1.5 Draft 2
// https://refspecs.linuxfoundation.org/elf/elf.pdf

use crate::error::{Result, RiscletError};
use std::collections::HashMap;

// ============================================================================
// ELF Constants
// ============================================================================

// ELF Identification
pub const EI_MAG0: u8 = 0x7f;
pub const EI_MAG1: u8 = b'E';
pub const EI_MAG2: u8 = b'L';
pub const EI_MAG3: u8 = b'F';
pub const EI_CLASS: u8 = 1; // ELFCLASS32
pub const EI_DATA: u8 = 1; // ELFDATA2LSB (little endian)
pub const EI_VERSION: u8 = 1; // EV_CURRENT
pub const EI_OSABI: u8 = 0; // ELFOSABI_SYSV
pub const EI_ABIVERSION: u8 = 0;

// ELF File Types
pub const ET_EXEC: u16 = 2; // Executable file

// Machine Type
pub const EM_RISCV: u16 = 0xF3; // RISC-V

// Object File Version
pub const EV_CURRENT: u32 = 1;

// ELF Header Flags (for RISC-V)
pub const EF_RISCV_FLOAT_ABI_DOUBLE: u32 = 0x4; // Double-precision FP ABI

// Section Types
pub const SHT_NULL: u32 = 0;
pub const SHT_PROGBITS: u32 = 1;
pub const SHT_SYMTAB: u32 = 2;
pub const SHT_STRTAB: u32 = 3;
pub const SHT_NOBITS: u32 = 8;
pub const SHT_RISCV_ATTRIBUTES: u32 = 0x7000_0003;

// Section Flags
pub const SHF_WRITE: u32 = 0x1;
pub const SHF_ALLOC: u32 = 0x2;
pub const SHF_EXECINSTR: u32 = 0x4;

// Program Header Types
pub const PT_LOAD: u32 = 1;
pub const PT_RISCV_ATTRIBUTES: u32 = 0x7000_0003;

// Program Header Flags
pub const PF_X: u32 = 0x1; // Execute
pub const PF_W: u32 = 0x2; // Write
pub const PF_R: u32 = 0x4; // Read

// Symbol Binding
pub const STB_LOCAL: u8 = 0;
pub const STB_GLOBAL: u8 = 1;

// Symbol Types
pub const STT_NOTYPE: u8 = 0;
pub const STT_SECTION: u8 = 3;
pub const STT_FILE: u8 = 4;

// Special Section Indices
pub const SHN_UNDEF: u16 = 0;
pub const SHN_ABS: u16 = 0xfff1;

// Header and entry sizes (32-bit ELF)
pub const ELF_HEADER_SIZE: u32 = 52;
pub const PROGRAM_HEADER_SIZE: u32 = 32;
pub const SYMBOL_ENTRY_SIZE: usize = 16;

// ============================================================================
// ELF Data Structures
// ============================================================================

/// ELF-32 File Header
#[derive(Debug, Clone)]
pub struct ElfHeader {
    pub e_ident: [u8; 16], // ELF identification
    pub e_type: u16,       // Object file type
    pub e_machine: u16,    // Machine type
    pub e_version: u32,    // Object file version
    pub e_entry: u32,      // Entry point address
    pub e_phoff: u32,      // Program header offset
    pub e_shoff: u32,      // Section header offset
    pub e_flags: u32,      // Processor-specific flags
    pub e_ehsize: u16,     // ELF header size
    pub e_phentsize: u16,  // Program header entry size
    pub e_phnum: u16,      // Number of program headers
    pub e_shentsize: u16,  // Section header entry size
    pub e_shnum: u16,      // Number of section headers
    pub e_shstrndx: u16,   // Section name string table index
}

impl Default for ElfHeader {
    fn default() -> Self {
        Self::new()
    }
}

impl ElfHeader {
    /// Create a new ELF header with standard RISC-V 32-bit values
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

        Self {
            e_ident,
            e_type: ET_EXEC,
            e_machine: EM_RISCV,
            e_version: EV_CURRENT,
            e_entry: 0,
            e_phoff: 52,
            e_shoff: 0,
            e_flags: EF_RISCV_FLOAT_ABI_DOUBLE,
            e_ehsize: 52,
            e_phentsize: 32,
            e_phnum: 0,
            e_shentsize: 40,
            e_shnum: 0,
            e_shstrndx: 0,
        }
    }

    /// Encode header to 52 bytes of little-endian binary
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(52);
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

    /// Decode header from 52 bytes of little-endian binary
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 52 {
            return Err(RiscletError::elf("ELF header too short".to_string()));
        }

        let mut e_ident = [0u8; 16];
        e_ident.copy_from_slice(&data[0..16]);

        Ok(Self {
            e_ident,
            e_type: u16::from_le_bytes([data[16], data[17]]),
            e_machine: u16::from_le_bytes([data[18], data[19]]),
            e_version: u32::from_le_bytes([
                data[20], data[21], data[22], data[23],
            ]),
            e_entry: u32::from_le_bytes([
                data[24], data[25], data[26], data[27],
            ]),
            e_phoff: u32::from_le_bytes([
                data[28], data[29], data[30], data[31],
            ]),
            e_shoff: u32::from_le_bytes([
                data[32], data[33], data[34], data[35],
            ]),
            e_flags: u32::from_le_bytes([
                data[36], data[37], data[38], data[39],
            ]),
            e_ehsize: u16::from_le_bytes([data[40], data[41]]),
            e_phentsize: u16::from_le_bytes([data[42], data[43]]),
            e_phnum: u16::from_le_bytes([data[44], data[45]]),
            e_shentsize: u16::from_le_bytes([data[46], data[47]]),
            e_shnum: u16::from_le_bytes([data[48], data[49]]),
            e_shstrndx: u16::from_le_bytes([data[50], data[51]]),
        })
    }
}

/// ELF-32 Program Header
#[derive(Debug, Clone)]
pub struct ElfProgramHeader {
    pub p_type: u32,   // Segment type
    pub p_offset: u32, // Segment file offset
    pub p_vaddr: u32,  // Segment virtual address
    pub p_paddr: u32,  // Segment physical address
    pub p_filesz: u32, // Segment size in file
    pub p_memsz: u32,  // Segment size in memory
    pub p_flags: u32,  // Segment flags
    pub p_align: u32,  // Segment alignment
}

impl ElfProgramHeader {
    /// Encode program header to 32 bytes of little-endian binary
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(&self.p_type.to_le_bytes());
        bytes.extend_from_slice(&self.p_offset.to_le_bytes());
        bytes.extend_from_slice(&self.p_vaddr.to_le_bytes());
        bytes.extend_from_slice(&self.p_paddr.to_le_bytes());
        bytes.extend_from_slice(&self.p_filesz.to_le_bytes());
        bytes.extend_from_slice(&self.p_memsz.to_le_bytes());
        bytes.extend_from_slice(&self.p_flags.to_le_bytes());
        bytes.extend_from_slice(&self.p_align.to_le_bytes());
        bytes
    }

    /// Decode program header from 32 bytes of little-endian binary
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 32 {
            return Err(RiscletError::elf(
                "Program header too short".to_string(),
            ));
        }

        Ok(Self {
            p_type: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            p_offset: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
            p_vaddr: u32::from_le_bytes([data[8], data[9], data[10], data[11]]),
            p_paddr: u32::from_le_bytes([
                data[12], data[13], data[14], data[15],
            ]),
            p_filesz: u32::from_le_bytes([
                data[16], data[17], data[18], data[19],
            ]),
            p_memsz: u32::from_le_bytes([
                data[20], data[21], data[22], data[23],
            ]),
            p_flags: u32::from_le_bytes([
                data[24], data[25], data[26], data[27],
            ]),
            p_align: u32::from_le_bytes([
                data[28], data[29], data[30], data[31],
            ]),
        })
    }
}

/// ELF-32 Section Header
#[derive(Debug, Clone)]
pub struct ElfSectionHeader {
    pub sh_name: u32,      // Section name (string table index)
    pub sh_type: u32,      // Section type
    pub sh_flags: u32,     // Section flags
    pub sh_addr: u32,      // Section virtual address
    pub sh_offset: u32,    // Section file offset
    pub sh_size: u32,      // Section size in bytes
    pub sh_link: u32,      // Link to another section
    pub sh_info: u32,      // Additional section information
    pub sh_addralign: u32, // Section alignment
    pub sh_entsize: u32,   // Entry size if section holds table
}

impl ElfSectionHeader {
    /// Create a null section header
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

    /// Encode section header to 40 bytes of little-endian binary
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(40);
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

    /// Decode section header from 40 bytes of little-endian binary
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 40 {
            return Err(RiscletError::elf(
                "Section header too short".to_string(),
            ));
        }

        Ok(Self {
            sh_name: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            sh_type: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
            sh_flags: u32::from_le_bytes([
                data[8], data[9], data[10], data[11],
            ]),
            sh_addr: u32::from_le_bytes([
                data[12], data[13], data[14], data[15],
            ]),
            sh_offset: u32::from_le_bytes([
                data[16], data[17], data[18], data[19],
            ]),
            sh_size: u32::from_le_bytes([
                data[20], data[21], data[22], data[23],
            ]),
            sh_link: u32::from_le_bytes([
                data[24], data[25], data[26], data[27],
            ]),
            sh_info: u32::from_le_bytes([
                data[28], data[29], data[30], data[31],
            ]),
            sh_addralign: u32::from_le_bytes([
                data[32], data[33], data[34], data[35],
            ]),
            sh_entsize: u32::from_le_bytes([
                data[36], data[37], data[38], data[39],
            ]),
        })
    }
}

/// ELF-32 Symbol Table Entry
#[derive(Debug, Clone)]
pub struct ElfSymbol {
    pub st_name: u32,  // Symbol name (string table index)
    pub st_value: u32, // Symbol value
    pub st_size: u32,  // Symbol size
    pub st_info: u8,   // Symbol type and binding
    pub st_other: u8,  // Symbol visibility
    pub st_shndx: u16, // Section index
}

impl ElfSymbol {
    /// Create undefined symbol (entry 0)
    pub fn null() -> Self {
        Self {
            st_name: 0,
            st_value: 0,
            st_size: 0,
            st_info: 0,
            st_other: 0,
            st_shndx: SHN_UNDEF,
        }
    }

    /// Create section symbol
    pub fn section(section_index: u16) -> Self {
        Self {
            st_name: 0,
            st_value: 0,
            st_size: 0,
            st_info: make_st_info(STB_LOCAL, STT_SECTION),
            st_other: 0,
            st_shndx: section_index,
        }
    }

    /// Create FILE symbol
    pub fn file(name_index: u32) -> Self {
        Self {
            st_name: name_index,
            st_value: 0,
            st_size: 0,
            st_info: make_st_info(STB_LOCAL, STT_FILE),
            st_other: 0,
            st_shndx: SHN_ABS,
        }
    }

    /// Encode symbol to 16 bytes of little-endian binary
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(16);
        bytes.extend_from_slice(&self.st_name.to_le_bytes());
        bytes.extend_from_slice(&self.st_value.to_le_bytes());
        bytes.extend_from_slice(&self.st_size.to_le_bytes());
        bytes.push(self.st_info);
        bytes.push(self.st_other);
        bytes.extend_from_slice(&self.st_shndx.to_le_bytes());
        bytes
    }

    /// Decode symbol from 16 bytes of little-endian binary
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 16 {
            return Err(RiscletError::elf(
                "Symbol entry too short".to_string(),
            ));
        }

        Ok(Self {
            st_name: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            st_value: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
            st_size: u32::from_le_bytes([data[8], data[9], data[10], data[11]]),
            st_info: data[12],
            st_other: data[13],
            st_shndx: u16::from_le_bytes([data[14], data[15]]),
        })
    }
}

/// Helper to create st_info field from binding and type
pub fn make_st_info(bind: u8, typ: u8) -> u8 {
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
    /// Create a new string table starting with a null byte
    pub fn new() -> Self {
        Self { strings: vec![0], offsets: HashMap::new() }
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

    /// Get the raw bytes of the string table
    pub fn data(&self) -> &[u8] {
        &self.strings
    }

    /// Get the length of the string table
    pub fn len(&self) -> usize {
        self.strings.len()
    }

    /// Check if the string table is empty (only null byte)
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.strings.len() <= 1
    }

    /// Parse a null-terminated string from the given offset
    pub fn get_string(&self, offset: usize) -> Result<String> {
        if offset >= self.strings.len() {
            return Err(RiscletError::elf(format!(
                "String offset {} out of bounds (table size: {})",
                offset,
                self.strings.len()
            )));
        }

        let mut end = offset;
        while end < self.strings.len() && self.strings[end] != 0 {
            end += 1;
        }

        if end >= self.strings.len() {
            return Err(RiscletError::elf(
                "Unterminated string in string table".to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&self.strings[offset..end]).into_owned())
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
/// For RV32IMACZifencei (I, M, A, C extensions + Zifencei), we generate:
/// "rv32i2p1_m2p0_a2p1_c2p0_zifencei2p0"
pub fn generate_riscv_attributes() -> Vec<u8> {
    // Generate attributes for RV32IMAC with compressed instructions and Zifencei
    let arch_string = "rv32i2p1_m2p0_a2p1_c2p0_zifencei2p0";

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
    attrs[length_pos..length_pos + 4]
        .copy_from_slice(&total_length.to_le_bytes());

    attrs
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Compute the expected combined size of the ELF header and program headers.
///
/// # Arguments
/// * `num_segments` - The number of program segments (e.g., 1 for text-only, 2 for text + data/bss).
///
/// # Returns
/// The total size in bytes (ELF header + program headers).
pub fn compute_header_size(num_segments: u32) -> u32 {
    ELF_HEADER_SIZE + (num_segments * PROGRAM_HEADER_SIZE)
}
