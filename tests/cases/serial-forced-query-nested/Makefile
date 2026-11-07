.PHONY: outer child sibling later
outer: child sibling
child: ; +@printf 'child\n' > child-ran; exit 1
sibling: ; +@printf 'sibling\n' > sibling-ran
later: ; +@printf 'later\n' > later-ran
