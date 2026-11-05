$(info read=[original] files=[$(MAKEFILE_LIST)] home=[$(HOME)] cli=[$(PAYLOAD)] count=[$(MAKE_RESTARTS)])
include generated.mk
generated.mk:;@printf 'READY := yes\n' > generated.mk
all:;@printf 'goal=[original:%s:%s]\n' '$(READY)' '$(PAYLOAD)'
