# The Textsmith (GTK version): shell integration for bash.
#
# Your prompt, your readline and your programs stay exactly as they are. On
# top of them this file adds:
#
# Invisible marks for the review buffer (F4) and the title bar:
#   OSC 133;A            the prompt is about to be shown
#   OSC 133;C            the command's output starts
#   OSC 133;D;<status>   the command finished
#   OSC 7                the current directory
#   OSC 7772;<text>      your prompt without colours, for the title bar
#   OSC 133;B            your typing starts here (end of the prompt)
#   OSC 7773;<line>      the line, as it will run, when a hook changed it
#
# Keys (The Textsmith sends the private sequences; the plain keys work in any
# terminal that loads this file):
#   Enter                expands #@name additions first; an error keeps the line
#   Alt+Enter            the line is a bookmark call, "name args": fill it in and run it
#   Tab                  after #@ completes bookmark names, otherwise as usual
#   \e[9981~             fill the bookmark call in without running it
#   \e[9982~             run the line with sudo
#   \e[9983~             bookmark the line (empty line: the last command)
#   \e[9984~             bookmark the line and run it
#   \e[9985~ / 9986~     put / run the line The Textsmith left in $TEXTSMITH_TMP/line
#   \e[9988~             add that text to the end of the line

[[ $- == *i* ]] || return
[[ -n $__ts_loaded ]] && return
__ts_loaded=1

[ -f ~/.bashrc ] && . ~/.bashrc

__ts_bin=${TEXTSMITH_BIN:-textsmith}

# ---------------------------------------------------------------- marks

if [[ "$(declare -p PROMPT_COMMAND 2>/dev/null)" == "declare -a"* ]]; then
  __ts_user_pc=("${PROMPT_COMMAND[@]}")
else
  __ts_user_pc=("${PROMPT_COMMAND-}")
fi
unset PROMPT_COMMAND

__ts_status() { return "$1"; }

__ts_precmd() {
  local __ts_ec=$? __ts_c
  printf '\033]133;D;%s\007' "$__ts_ec"
  history -a 2>/dev/null                     # saved now, so new tabs see it
  for __ts_c in "${__ts_user_pc[@]}"; do
    [[ -n $__ts_c ]] && { __ts_status "$__ts_ec"; eval "$__ts_c"; }
  done
  # The prompt for the title bar. A prompt that runs commands ($(...)) is
  # left alone, so they do not run twice; the title then uses user@host: dir.
  if [[ -n $PS1 && $PS1 != *'$('* && $PS1 != *'`'* ]]; then
    local __ts_p=${PS1@P}
    __ts_p=${__ts_p//$'\e]'*([^$'\a'])$'\a'/}        # title-setting sequences
    __ts_p=${__ts_p//$'\e['*([0-9;?])[A-Za-z]/}       # colours
    __ts_p=${__ts_p//[$'\001\002\e\a']/}
    __ts_p=${__ts_p//$'\n'/ }
    printf '\033]7772;%s\007' "$__ts_p"
  else
    printf '\033]7772;\007'
  fi
  # Where your typing starts (133;B), at the end of your prompt.
  [[ $PS1 == *'133;B'* ]] || PS1+='\[\e]133;B\a\]'
  printf '\033]7;file://%s%s\007\033]133;A\007' "${HOSTNAME:-localhost}" "$PWD"
  __ts_status "$__ts_ec"
}
shopt -s extglob
PROMPT_COMMAND=__ts_precmd
PS0="${PS0-}"$'\e]133;C\a'

# ---------------------------------------------------------------- keys

# The key that follows each hook: accept the line, or do nothing.
__ts_next() {
  local f=redraw-current-line m
  [[ $1 == accept ]] && f=accept-line
  [[ $1 == tab ]] && f=$__ts_tabfunc
  [[ $1 == list ]] && f=possible-completions
  for m in emacs vi-insert vi-command; do bind -m $m "\"\\e[9991~\": $f"; done
}

# A sentence for you: printed on its own line, then readline draws the prompt again.
__ts_say() {
  printf '\r\033[K%s\n' "$1"
}

# Enter: #@ additions are filled in; the line goes to the review buffer.
__ts_enter() {
  local out rc
  if [[ $READLINE_LINE == *'#@'* ]]; then
    out=$("$__ts_bin" bm expand "$READLINE_LINE"); rc=$?
    if (( rc == 0 )); then
      READLINE_LINE=$out; READLINE_POINT=${#out}
    elif (( rc == 1 )); then
      __ts_say "$out"; __ts_next none; return
    fi
  fi
  printf '\033]7773;%s\007' "${READLINE_LINE//[$'\a\e']/}"
  __ts_next accept
}

# Alt+Enter: "name args" is a bookmark call.
__ts_call() {
  local out
  if out=$("$__ts_bin" bm call "$READLINE_LINE"); then
    READLINE_LINE=$out; READLINE_POINT=${#out}
    [[ $1 == fill ]] && { __ts_next none; return; }
    printf '\033]7773;%s\007' "${READLINE_LINE//[$'\a\e']/}"
    __ts_next accept
  else
    __ts_say "$out"; __ts_next none
  fi
}

__ts_sudo() {
  [[ $READLINE_LINE =~ ^[[:space:]]*sudo[[:space:]] ]] || READLINE_LINE="sudo $READLINE_LINE"
  READLINE_POINT=${#READLINE_LINE}
}

__ts_last_command() {
  local l
  l=$(HISTTIMEFORMAT='' history 1)              # "  42  command" ("42*" if edited)
  l=${l##+([[:space:]])+([0-9])?(\*)  }
  printf '%s' "$l"
}

# Bookmark the line, or the last command when the line is empty.
__ts_bookmark() {
  local line=$READLINE_LINE out
  [[ -z ${line//[[:space:]]/} ]] && line=$(__ts_last_command)
  out=$("$__ts_bin" bm add "$line") && { READLINE_LINE=''; READLINE_POINT=0; }
  __ts_say "$out"
}

__ts_bookmark_run() {
  local line=$READLINE_LINE out msg act text
  [[ -z ${line//[[:space:]]/} ]] && line=$(__ts_last_command)
  out=$("$__ts_bin" bm add-run "$line")
  msg=${out%%$'\n'*}; out=${out#*$'\n'}
  act=${out%%$'\n'*}; text=${out#*$'\n'}
  [[ $act == "$out" ]] && text=''
  __ts_say "$msg"
  case $act in
    run)  READLINE_LINE=$text; READLINE_POINT=${#text}
          printf '\033]7773;%s\007' "${text//[$'\a\e']/}"; __ts_next accept ;;
    edit) READLINE_LINE=$text; READLINE_POINT=${#text}; __ts_next none ;;
    *)    __ts_next none ;;
  esac
}

# A line from The Textsmith (the bookmark list).
__ts_put() {
  local f=${TEXTSMITH_TMP:-/nonexistent}/line t
  [[ -r $f ]] || return
  t=$(<"$f")
  if [[ $1 == append ]]; then
    [[ -n $READLINE_LINE && $READLINE_LINE != *[[:space:]] ]] && READLINE_LINE+=' '
    READLINE_LINE+=$t
  else
    READLINE_LINE=$t
  fi
  READLINE_POINT=${#READLINE_LINE}
}

# Tab: bookmark names after #@ (or after @ further along), otherwise your
# usual completion. Readline lists the choices on a second Tab only when the
# previous key was Tab, which a hook in between hides, so that is noticed
# here: a second Tab with nothing changed lists them.
__ts_tab() {
  local before=${READLINE_LINE:0:READLINE_POINT} out rc new msg state
  if [[ $before =~ (^|[[:space:]])#?@[^[:space:]]*$ ]]; then
    out=$("$__ts_bin" bm complete "$before"); rc=$?
    if (( rc == 0 )); then
      new=${out%%$'\n'*}; msg=
      [[ $out == *$'\n'* ]] && msg=${out#*$'\n'}
      READLINE_LINE=$new${READLINE_LINE:READLINE_POINT}; READLINE_POINT=${#new}
      [[ -n $msg ]] && __ts_say "$msg"
      __ts_tabstate=; __ts_next none; return
    fi
  fi
  if [[ "$READLINE_POINT:$READLINE_LINE" == "$__ts_tabstate" ]]; then
    __ts_next list
  else
    __ts_next tab
  fi
}
# Runs after the completion: what the line looks like now.
__ts_tab_after() { __ts_tabstate="$READLINE_POINT:$READLINE_LINE"; }

__ts_bind() {
  local m=$1
  bind -m $m -x '"\e[9990~": __ts_enter'
  bind -m $m -x '"\e[9992~": __ts_call'
  bind -m $m -x '"\e[9993~": __ts_call fill'
  bind -m $m -x '"\e[9995~": __ts_sudo'
  bind -m $m -x '"\e[9996~": __ts_bookmark'
  bind -m $m -x '"\e[9997~": __ts_bookmark_run'
  bind -m $m -x '"\e[9987~": __ts_put'
  bind -m $m -x '"\e[9988~": __ts_put append'
  bind -m $m '"\e[9991~": accept-line'
  bind -m $m '"\e[9989~": "\e[9990~\e[9991~"'
  # Inside The Textsmith, Enter stays readline's own: a hook on it makes
  # readline draw the prompt again, which a screen reader then reads out.
  # The Textsmith reads the line from the screen and sends \e[9989~ only
  # when it holds #@. In other terminals Enter runs the hook itself.
  if [[ -z $TEXTSMITH_GTK ]]; then
    bind -m $m '"\C-m": "\e[9990~\e[9991~"'
    bind -m $m '"\C-j": "\e[9990~\e[9991~"'
  fi
  bind -m $m '"\e[9980~": "\e[9992~\e[9991~"'
  bind -m $m '"\e[9981~": "\e[9993~\e[9991~"'
  bind -m $m '"\e[9982~": "\e[9995~\e[9990~\e[9991~"'
  bind -m $m '"\e[9983~": "\e[9996~"'
  bind -m $m '"\e[9984~": "\e[9997~\e[9991~"'
  bind -m $m '"\e[9985~": "\e[9987~"'
  bind -m $m '"\e[9986~": "\e[9987~\e[9990~\e[9991~"'
  if [[ $m != vi-command ]]; then
    bind -m $m '"\e\C-m": "\e[9992~\e[9991~"'
    bind -m $m '"\e\C-j": "\e[9992~\e[9991~"'
  fi
}

# Tab is taken over only when it does plain completion (the default), so a
# menu-complete or fzf setup of your own is left as it is.
__ts_tabfunc=complete
if bind -m emacs -p 2>/dev/null | grep -qx '"\\C-i": complete' && ! bind -X 2>/dev/null | grep -q '^"\\C-i"'; then
  __ts_tabhook=1
fi
for __ts_m in emacs vi-insert vi-command; do
  __ts_bind $__ts_m
  # Inside The Textsmith, Tab stays readline's own (a hook would make readline
  # draw the prompt again, which a screen reader reads); The Textsmith sends
  # \e[9978~ instead when the word is a #@ name.
  if [[ -n $TEXTSMITH_GTK && $__ts_m != vi-command ]]; then
    bind -m $__ts_m -x '"\e[9998~": __ts_tab'
    bind -m $__ts_m '"\e[9978~": "\e[9998~\e[9991~"'
    # editing.tabCompletion "cycle": Tab puts in the first choice and each Tab
    # after it the next, Shift+Tab goes back, as zsh does. Only when Tab still
    # does bash's plain completion, so a setup of your own is left alone.
    if [[ $TEXTSMITH_TAB == cycle && -n $__ts_tabhook ]]; then
      bind -m $__ts_m '"\C-i": menu-complete'
      bind -m $__ts_m '"\e[Z": menu-complete-backward'
      __ts_tabfunc=menu-complete
    fi
  elif [[ -n $__ts_tabhook && $__ts_m != vi-command ]]; then
    bind -m $__ts_m -x '"\e[9998~": __ts_tab'
    bind -m $__ts_m -x '"\e[9979~": __ts_tab_after'
    bind -m $__ts_m '"\C-i": "\e[9998~\e[9991~\e[9979~"'
  fi
done
unset __ts_m

# Program-specific completions (git, systemctl...) come from bash-completion.
if ! declare -F _completion_loader >/dev/null && ! declare -F _comp_load >/dev/null; then
  for __ts_f in /usr/share/bash-completion/bash_completion /etc/bash_completion; do
    [[ -r $__ts_f ]] && { . "$__ts_f"; break; }
  done
  unset __ts_f
fi
