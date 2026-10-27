$(info shell=<$(shell /bin/sh -c 'kill -s QUIT $$$$; printf "survived\n"')>)
all:;@/bin/sh -c 'kill -s QUIT $$$$; printf "survived\n"'
