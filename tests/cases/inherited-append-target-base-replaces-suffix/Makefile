X = $(file >>seen,base)$(eval chosen: X := changed)outer
chosen: X += $(file >>seen,suffix)tail
chosen: ; $(info first=[$(X)] second=[$(X)] raw=[$(value X)])@:
