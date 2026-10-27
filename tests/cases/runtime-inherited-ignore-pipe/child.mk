$(info shell=<$(shell /bin/sh -c 'kill -s PIPE $$$$; printf "survived\n"')>)
all:;@/bin/sh -c 'kill -s PIPE $$$$; printf "survived\n"'
