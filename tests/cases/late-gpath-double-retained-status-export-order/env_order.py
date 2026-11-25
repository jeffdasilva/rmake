import os
for name, value in os.environ.items():
    if name in ('.SHELLSTATUS', 'ANCHOR_3190'):
        print(f'{name}|{value}')
