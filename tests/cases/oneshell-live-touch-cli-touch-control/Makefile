.ONESHELL:
EMPTY :=
$(file >events,read)
export CHILD = $(file >>events,export)yes
SHELL = $(file >>events,shell)/bin/sh
.SHELLFLAGS = $(file >>events,flags)-c
IFS = $(file >>events,ifs)
.PHONY: force
force:
target: force
	$(file >>events,body)printf "%s\n" body > child
