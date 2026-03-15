" Vim syntax file for ilo
" Language: ilo
" Maintainer: ilo-lang
" URL: https://github.com/ilo-lang/ilo

if exists("b:current_syntax")
  finish
endif

let b:current_syntax = "ilo"

" Comments: -- to end of line
syntax match iloComment "--.*$"

" Strings: "..." (no interpolation at lex level — {} inside strings is literal)
syntax region iloString start=/"/ skip=/\\"/ end=/"/ contains=iloEscape
syntax match iloEscape /\\[ntr"\\]/ contained

" Numbers: integers and floats, including negative
syntax match iloNumber "-\?\d\+\(\.\d\+\)\?"

" Boolean literals
syntax keyword iloBoolean true false

" Nil literal
syntax keyword iloNil nil

" Keywords
syntax keyword iloKeyword type tool use with timeout retry

" Control flow
syntax keyword iloControl wh brk cnt ret
syntax match iloControl "@"

" Type constructors (standalone uppercase letters used as types)
syntax match iloType "\<\(L\|R\|F\|O\|M\|S\)\>"

" Primitive type annotations (after : in parameter/return position)
" Match :n :t :b and >n >t >b patterns
syntax match iloType ":\(n\|t\|b\|number\|text\|bool\)\>"
syntax match iloType ">\(n\|t\|b\|number\|text\|bool\)\>"

" Operators: multi-char first (greedy)
syntax match iloOperator ">="
syntax match iloOperator "<="
syntax match iloOperator "!="
syntax match iloOperator "+="
syntax match iloOperator ">>"
syntax match iloOperator "??"
syntax match iloOperator "\.\."
syntax match iloOperator "\.?"

" Single-char operators (excludes ? which is highlighted as iloControl)
syntax match iloOperator "[-+*/><&|!^~$]"

" Builtins — all canonical names from builtins.rs
syntax keyword iloBuiltin str num abs flr cel rou min max mod sum avg
syntax keyword iloBuiltin len hd tl rev srt slc unq flat has spl cat
syntax keyword iloBuiltin map flt fld grp rnd now
syntax keyword iloBuiltin rd rdl rdb wr wrl prnt env
syntax keyword iloBuiltin trm fmt rgx
syntax keyword iloBuiltin jpth jdmp jpar
syntax keyword iloBuiltin get post
syntax keyword iloBuiltin mmap mget mset mhas mkeys mvals mdel

" Function declarations: identifier at start of line (before params)
" ilo functions start at column 0 followed by space or param/return syntax
syntax match iloFunction "^\([a-z][a-z0-9]*\(-[a-z0-9]\+\)*\)\ze\s*[a-z:>]"
syntax match iloFunction "^\([a-z][a-z0-9]*\(-[a-z0-9]\+\)*\)\ze>"

" Type definitions: identifier after 'type' keyword
syntax match iloTypeDef "\<type\s\+\zs[a-z][a-z0-9-]*"

" Tool declarations: identifier after 'tool' keyword
syntax match iloFunction "\<tool\s\+\zs[a-z][a-z0-9-]*"

" Match expression: ? followed by identifier
syntax match iloControl "?"

" Highlight links to standard groups
hi def link iloComment     Comment
hi def link iloString      String
hi def link iloEscape      SpecialChar
hi def link iloNumber      Number
hi def link iloBoolean     Boolean
hi def link iloNil         Constant
hi def link iloKeyword     Keyword
hi def link iloControl     Conditional
hi def link iloType        Type
hi def link iloOperator    Operator
hi def link iloBuiltin     Function
hi def link iloFunction    Function
hi def link iloTypeDef     Type
