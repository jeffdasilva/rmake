V := global
VPATH := pool
GPATH := pool
all: left right caller
left: V := first
right: V := second
left: dest
right: pool/dest
caller:;@$(eval pool/dest: RESULT := $$(info retained=[$$(V)|$$@])):
