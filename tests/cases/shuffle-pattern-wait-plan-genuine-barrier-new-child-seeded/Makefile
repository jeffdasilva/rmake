all.out: d01 d02 d03 d04 d05 d06 d07 d08 d09 d10 d11
%.out: p1 .WAIT p2 ; @printf 'recipe %s first=%s all=%s repeated=%s order=%s\n' '$@' '$<' '$^' '$+' '$|'; printf 'recipe %s\n' '$@' >>seen
d01 d02 d03 d04 d05 d06 d07 d08 d09 d10 d11 p1 p2 p3: ; @printf 'leaf %s\n' '$@'; printf '%s\n' '$@' >>seen; printf '%s\n' '$@' >'$@.effect'
p1: c01 c02 c03 c04 c05 c06 c07 c08 c09 c10 c11
c01 c02 c03 c04 c05 c06 c07 c08 c09 c10 c11: ; @printf 'child %s\n' '$@'; printf '%s\n' '$@' >>seen
