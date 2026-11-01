# The Textsmith (GTK version): zsh integration.
#
# Your prompt, line editor and programs stay as they are. On top of them:
#
#   OSC 133;A / 133;C / 133;D;<status>   prompt shown / output starts / finished
#   OSC 7 directory, OSC 7772 your prompt for the title bar, OSC 7773 the line
#   that runs (after #@ expansion), for the review buffer (F4)
#
#   Enter       expands #@name additions first; an error keeps the line
#   Alt+Enter   the line is a bookmark call, "name args": fill it in and run it
#   Tab         after #@ completes bookmark names, otherwise as usual
#   and the private keys \e[9981~ to \e[9986~ described in textsmith.bashrc.
#
# Your own .zshrc is read first, so your options, plugins and completions apply.

ZDOTDIR=${TEXTSMITH_USER_ZDOTDIR:-$HOME}
[[ -r $ZDOTDIR/.zshrc ]] && source "$ZDOTDIR/.zshrc"

# Bookmarks are typed as comments: #name: command
setopt INTERACTIVE_COMMENTS

__ts_bin=${TEXTSMITH_BIN:-textsmith}

# ---------------------------------------------------------------- marks

__ts_first() {                          # runs first: $? is still the command's
  local ec=$?
  print -rn -- $'\e]133;D;'$ec$'\a'
  return $ec
}
__ts_last() {                           # runs last, after your own precmd hooks
  local ec=$?
  setopt localoptions extendedglob
  fc -AI 2>/dev/null                    # history saved now, so new tabs see it
  local p=
  if [[ -n $PS1 ]] && ! [[ -o PROMPT_SUBST && ( $PS1 == *'$('* || $PS1 == *'`'* ) ]]; then
    p=${(%)PS1}
    p=${p//$'\e]'[^$'\a']#$'\a'/}        # title-setting sequences
    p=${p//$'\e['[0-9;?]#[A-Za-z]/}      # colours
    p=${p//[$'\e\a']/}
    p=${p//$'\n'/ }
  fi
  print -rn -- $'\e]7772;'$p$'\a\e]7;file://'$HOST$PWD$'\a\e]133;A\a'
  # Where your typing starts (133;B), at the end of your prompt.
  [[ $PS1 == *'133;B'* ]] || PS1+=$'%{\e]133;B\a%}'
  return $ec
}
__ts_preexec() { print -rn -- $'\e]133;C\a' }
precmd_functions=(__ts_first ${precmd_functions:#__ts_*} __ts_last)
preexec_functions=(${preexec_functions:#__ts_*} __ts_preexec)

# ---------------------------------------------------------------- keys

# A sentence for you, on its own line above the prompt.
__ts_say() { zle -I; print -r -- "$1" }
__ts_mark() { print -rn -- $'\e]7773;'${BUFFER//[$'\a\e']/}$'\a' }

zle -A accept-line __ts_orig_accept 2>/dev/null || zle -A .accept-line __ts_orig_accept

__ts_accept() {
  local out rc
  if [[ $BUFFER == *'#@'* ]]; then
    out=$("$__ts_bin" bm expand "$BUFFER"); rc=$?
    if (( rc == 0 )); then
      BUFFER=$out; CURSOR=$#BUFFER
    elif (( rc == 1 )); then
      __ts_say "$out"; return
    fi
  fi
  __ts_mark
  zle __ts_orig_accept
}
zle -N accept-line __ts_accept

__ts_call() {
  local out
  if out=$("$__ts_bin" bm call "$BUFFER"); then
    BUFFER=$out; CURSOR=$#BUFFER
    [[ $WIDGET == __ts-fill ]] && return
    __ts_mark
    zle __ts_orig_accept
  else
    __ts_say "$out"
  fi
}
zle -N __ts-call __ts_call
zle -N __ts-fill __ts_call

__ts_sudo() {
  [[ $BUFFER == [[:space:]]#sudo[[:space:]]* ]] || BUFFER="sudo $BUFFER"
  CURSOR=$#BUFFER
  __ts_accept
}
zle -N __ts-sudo __ts_sudo

__ts_bookmark() {
  local line=$BUFFER out msg act text
  [[ -z ${line//[[:space:]]/} ]] && line=$(fc -ln -1 2>/dev/null)
  if [[ $WIDGET == __ts-bookmark ]]; then
    out=$("$__ts_bin" bm add "$line") && BUFFER=''
    __ts_say "$out"
    return
  fi
  out=$("$__ts_bin" bm add-run "$line")
  msg=${out%%$'\n'*}; out=${out#*$'\n'}
  act=${out%%$'\n'*}; text=${out#*$'\n'}
  [[ $act == "$out" ]] && text=''
  __ts_say "$msg"
  case $act in
    run)  BUFFER=$text; CURSOR=$#BUFFER; __ts_mark; zle __ts_orig_accept ;;
    edit) BUFFER=$text; CURSOR=$#BUFFER ;;
  esac
}
zle -N __ts-bookmark __ts_bookmark
zle -N __ts-bookmark-run __ts_bookmark

__ts_put() {
  local f=${TEXTSMITH_TMP:-/nonexistent}/line
  [[ -r $f ]] || return
  if [[ $WIDGET == __ts-put-append ]]; then
    [[ -n $BUFFER && $BUFFER != *[[:space:]] ]] && BUFFER+=' '
    BUFFER+=$(<"$f")
  else
    BUFFER=$(<"$f")
  fi
  CURSOR=$#BUFFER
  [[ $WIDGET == __ts-put-run ]] && __ts_accept
}
zle -N __ts-put __ts_put
zle -N __ts-put-run __ts_put
zle -N __ts-put-append __ts_put

# Tab: bookmark names after #@, otherwise whatever Tab did before.
__ts_tabw=${${(z)"$(bindkey -M main '^I')"}[2]}
[[ -z $__ts_tabw || $__ts_tabw == undefined-key ]] && __ts_tabw=expand-or-complete
__ts_tab() {
  local out new msg
  if [[ $LBUFFER =~ '(^|[[:space:]])#?@[^[:space:]]*$' ]]; then
    out=$("$__ts_bin" bm complete "$LBUFFER")
    if (( $? == 0 )); then
      new=${out%%$'\n'*}; msg=
      [[ $out == *$'\n'* ]] && msg=${out#*$'\n'}
      LBUFFER=$new
      [[ -n $msg ]] && __ts_say "$msg"
      return
    fi
  fi
  zle $__ts_tabw
}
zle -N __ts-tab __ts_tab

() {
  local m
  for m in emacs viins vicmd; do
    bindkey -M $m $'\e[9989~' accept-line
    bindkey -M $m '^[^M' __ts-call '^[^J' __ts-call $'\e[9980~' __ts-call $'\e[9981~' __ts-fill \
      $'\e[9982~' __ts-sudo $'\e[9983~' __ts-bookmark $'\e[9984~' __ts-bookmark-run \
      $'\e[9985~' __ts-put $'\e[9986~' __ts-put-run $'\e[9988~' __ts-put-append
  done
  # Inside The Textsmith Tab stays your own; it sends \e[9978~ for a #@ name.
  for m in emacs viins; do bindkey -M $m $'\e[9978~' __ts-tab; done
  # fzf-tab and the like keep Tab to themselves.
  if [[ -z $TEXTSMITH_GTK && $__ts_tabw != fzf-* ]]; then
    for m in emacs viins; do bindkey -M $m '^I' __ts-tab; done
  fi
}
