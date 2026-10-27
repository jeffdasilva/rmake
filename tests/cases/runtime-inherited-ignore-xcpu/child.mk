$(info shell=<$(shell /bin/sh -c 'kill -s XCPU $$$$; printf "survived\n"')>)
all:;@/bin/sh -c 'kill -s XCPU $$$$; printf "survived\n"'
