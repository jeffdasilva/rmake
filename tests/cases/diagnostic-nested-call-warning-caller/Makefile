INNER = $(warning inner[$(1)])
OUTER = $(call INNER,$(1))$(warning outer[$(1)])
all:
	@$(call OUTER,value)printf 'continued\n'
