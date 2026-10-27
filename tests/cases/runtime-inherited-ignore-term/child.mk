$(info shell=<$(shell /bin/sh -c 'kill -s TERM $$$$; printf "survived\n"')>)
all:;@/bin/sh -c 'kill -s TERM $$$$; printf "survived\n"'
