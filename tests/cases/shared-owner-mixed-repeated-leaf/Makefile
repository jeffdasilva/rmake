%.out: %.mid %.tmp ; @printf '%s\n' 'recipe|$@|$<|$^|$+|$?|$|'
%.mid: %.in %.in | %.gate %.gate ; @printf '%s\n' 'recipe|$@|$<|$^|$+|$?|$|'
%.tmp: %.mid ; @printf '%s\n' 'recipe|$@|$<|$^|$+|$?|$|'
