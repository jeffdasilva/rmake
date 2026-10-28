guile=GLOBAL:$(1)
all: parent other
parent: private guile=PARENT:$(1)
parent: child
parent child other: ; @$(info $@=[$(call guile,ARG)])true
