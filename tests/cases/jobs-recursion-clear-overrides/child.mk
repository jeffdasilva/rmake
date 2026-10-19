TOKEN=file
all:;@printf "%s|%s|%s\n" '$(TOKEN)' '$(origin TOKEN)' '$(flavor TOKEN)'
