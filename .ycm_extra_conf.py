def Settings(**kwargs):
    if kwargs['language'] == 'rust':
        return {
            'ls': {
                'diagnostics': {
                    'disabled': ['unresolved-proc-macro', 'inactive-code']
                },
                'checkOnSave': {
                    'enable': True,
                    'extraArgs': ['--target-dir', 'target/check']
                },
            }
        }
