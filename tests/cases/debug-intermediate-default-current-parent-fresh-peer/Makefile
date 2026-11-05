Makefile:;
.DEFAULT:; @$(file >>seen,default:$@)$(file >$@,artifact)printf 'default:%s\n' '$@'
.INTERMEDIATE: scratch
$(shell touch -t 202001010000 parent; touch -t 202001010001 trigger)
trigger:;
parent: scratch trigger; @$(file >>seen,parent:$(file <scratch))printf 'parent:%s\n' '$(file <scratch)'
