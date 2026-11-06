$(info read=[alternate] files=[$(MAKEFILE_LIST)] home=[$(HOME)] cli=[$(PAYLOAD)] count=[$(MAKE_RESTARTS)])
READY := wrong
all:;@printf 'goal=[alternate:%s:%s]\n' '$(READY)' '$(PAYLOAD)'
