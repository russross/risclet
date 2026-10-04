.PHONY: all run trace debug clean

SOURCES = start.s sort.s print.s

all: run

run:
	risclet run --check-abi $(SOURCES)

trace:
	risclet trace $(SOURCES)

debug:
	risclet debug $(SOURCES)

clean:
	rm -f core *.o *.out
