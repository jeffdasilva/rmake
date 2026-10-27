$(info shell=<$(shell /bin/sh -c 'kill -s HUP $$$$; printf "survived\n"')>)
all:;@/bin/sh -c 'kill -s HUP $$$$; printf "survived\n"'
