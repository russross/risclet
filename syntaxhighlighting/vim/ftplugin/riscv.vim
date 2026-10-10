" Comment and word settings follow the assembler tokenizer.
if exists("b:did_ftplugin")
  finish
endif
let b:did_ftplugin = 1
setlocal commentstring=#\ %s
setlocal comments=:#
setlocal iskeyword=@,48-57,_,36,46
let b:undo_ftplugin = "setlocal commentstring< comments< iskeyword<"
