%.out: %.mid %.tmp ; @cat seen-a.mid seen-a.tmp; printf '%s\n' 'recipe|$@|$<|$^|$+|$?|$|'
%.mid: %.in ; @printf '%s\n' 'recipe|$@|$<|$^|$+|$?|$|' > seen-a.mid
%.tmp: %.mid ; @printf '%s\n' 'recipe|$@|$<|$^|$+|$?|$|' > seen-a.tmp
