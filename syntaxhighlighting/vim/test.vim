" Run from the repository root: vim -Nu NONE -n -es -S vim/test.vim
source vim/syntax/risclet.vim
let s:cases = json_decode(join(readfile('tests/highlighting.json'), "\n"))
call setline(1, map(copy(s:cases), 'v:val.line'))

" Check actual syntax groups, including line recovery after malformed literals.
for s:index in range(len(s:cases))
  for s:check in s:cases[s:index].checks
    call assert_equal('risclet' . s:check[1],
          \ synIDattr(synID(s:index + 1, s:check[0] + 1, 1), 'name'),
          \ s:cases[s:index].line)
  endfor
endfor
if !empty(v:errors)
  call writefile(v:errors, '/dev/stderr')
  cquit
endif
qa!
