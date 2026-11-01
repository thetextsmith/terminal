# The Textsmith: your .zprofile, when zsh runs as a login shell.
ZDOTDIR=${TEXTSMITH_USER_ZDOTDIR:-$HOME}
[[ -r $ZDOTDIR/.zprofile ]] && source "$ZDOTDIR/.zprofile"
ZDOTDIR=${0:h}
