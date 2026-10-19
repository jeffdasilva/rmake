await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done
all: left right
left: d0a d0b
right: rightleaf
d0a: d1a d1b
d0b: d1a d1b
d1a: d2a d2b
d1b: d2a d2b
d2a: d3a d3b
d2b: d3a d3b
d3a: d4a d4b
d3b: d4a d4b
d4a: d5a d5b
d4b: d5a d5b
d5a: d6a d6b
d5b: d6a d6b
d6a: d7a d7b
d6b: d7a d7b
d7a: d8a d8b
d7b: d8a d8b
d8a: d9a d9b
d8b: d9a d9b
d9a: d10a d10b
d9b: d10a d10b
d10a: d11a d11b
d10b: d11a d11b
d11a: d12a d12b
d11b: d12a d12b
d12a: d13a d13b
d12b: d13a d13b
d13a: d14a d14b
d13b: d14a d14b
d14a: d15a d15b
d14b: d15a d15b
d15a: d16a d16b
d15b: d16a d16b
d16a: d17a d17b
d16b: d17a d17b
d17a: d18a d18b
d17b: d18a d18b
d18a: d19a d19b
d18b: d19a d19b
d19a: d20a d20b
d19b: d20a d20b
d20a: d21a d21b
d20b: d21a d21b
d21a: d22a d22b
d21b: d22a d22b
d22a: d23a d23b
d22b: d23a d23b
d23a: leftleaf
d23b: leftleaf
leftleaf:;@touch left.started;$(call await,right.started);touch $@
rightleaf:;@touch right.started;$(call await,left.started);touch $@
