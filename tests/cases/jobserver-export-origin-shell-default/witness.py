import os
for name in ('MAKEFLAGS', 'MFLAGS'):
    flags = os.environ.get(name)
    if flags is None:
        state = 'missing'
    elif '--jobserver-auth=-2,-2' in flags.split():
        state = 'disabled'
    elif '--jobserver-auth=' in flags:
        state = 'enabled'
    else:
        state = 'absent'
    print(f'{name}:{state}')
