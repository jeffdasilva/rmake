all:
	+@cc -shared -fPIC -O2 -o clock.so clock.c -ldl
	+@LD_PRELOAD=./clock.so $(MAKE) -f inner.mk --no-print-directory all
