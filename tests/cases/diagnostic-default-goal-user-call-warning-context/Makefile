INNER = $(warning inner[$(1)])
OUTER = $(warning before)$(call INNER,$(warning argument)value)$(warning after)all
.DEFAULT_GOAL = $(call OUTER)
all:
	@true
