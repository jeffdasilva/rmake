$(info shell=<$(shell /bin/sh -c 'kill -s XFSZ $$$$; printf "survived\n"')>)
all:;@/bin/sh -c 'kill -s XFSZ $$$$; printf "survived\n"'
