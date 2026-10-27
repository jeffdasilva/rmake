$(info shell=<$(shell /bin/sh -c 'kill -s INT $$$$; printf "survived\n"')>)
all:;@/bin/sh -c 'kill -s INT $$$$; printf "survived\n"'
