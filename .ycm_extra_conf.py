def Settings(**kwargs):
    if kwargs['language'] == 'rust':
        return {
            'diagnostics_disabled': ['unresolved-proc-macro', 'inactive-code']
        }
