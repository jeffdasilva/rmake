$(info read=[original] files=[$(MAKEFILE_LIST)] home=[$(HOME)] cli=[$(PAYLOAD)] count=[$(MAKE_RESTARTS)])
READY := yes
all:;@printf 'goal=[original:%s:%s]\n' '$(READY)' '$(PAYLOAD)'
