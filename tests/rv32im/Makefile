.PHONY: all clean

# RISC-V toolchain (64-bit tools for cross-compilation to 32-bit)
AS = riscv64-unknown-elf-as
LD = riscv64-unknown-elf-ld
OBJDUMP = riscv64-unknown-elf-objdump

# Assembler flags for RV32IM (32-bit RISC-V with Integer and Multiply/Divide extensions)
ASFLAGS = -march=rv32im -mabi=ilp32 -g

# Linker flags - specify 32-bit little-endian RISC-V target
LDFLAGS = -m elf32lriscv -N

# Source files
SOURCES = $(wildcard test_*.s)

# Object files (from sources)
OBJECTS = $(SOURCES:.s=.o)

# Executables
EXECUTABLES = $(SOURCES:.s=.elf)

# All targets
all: $(EXECUTABLES)

# Rule to assemble .s to .o
%.o: %.s
	$(AS) $(ASFLAGS) $< -o $@

# Rule to link .o to .elf executable
%.elf: %.o
	$(LD) $(LDFLAGS) -Ttext 0x1000 $< -o $@

# Rule to generate disassembly for each test
%.dis: %.elf
	$(OBJDUMP) -d $< > $@

# Generate all disassemblies
disassemble: $(EXECUTABLES:.elf=.dis)

# Clean up
clean:
	rm -f $(OBJECTS) $(EXECUTABLES) $(EXECUTABLES:.elf=.dis)

# Show contents of a test for inspection
show-%: test_%.elf
	$(OBJDUMP) -d $< | head -50
