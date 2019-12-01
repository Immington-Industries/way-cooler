project(
    'way-cooler',
    'c',
    version: '0.8.0',
    license: 'MIT',
    meson_version: '>=0.48.0',
    default_options: [
        'c_std=c11',
        'warning_level=2',
        'werror=true',
    ]
)

add_project_arguments(
    [
        '-DWLR_USE_UNSTABLE',

        '-Wno-unused-parameter',
        '-Wno-unused-result',
        '-Wundef',
        '-Wvla',
    ],
    language: 'c'
)

git = find_program('git', native: true, required: false)

if git.found()
	git_commit_hash = run_command([git.path(), 'describe', '--always', '--tags'])
	git_branch = run_command([git.path(), 'rev-parse', '--abbrev-ref', 'HEAD'])
	if git_commit_hash.returncode() == 0 and git_branch.returncode() == 0
		version = '"@0@ (" __DATE__ ", branch \'@1@\')"'.format(git_commit_hash.stdout().strip(), git_branch.stdout().strip())
	endif
endif
add_project_arguments('-DWAY_COOLER_VERSION=@0@'.format(version), language: 'c')

way_cooler_includes = include_directories('include')

subdir('protocols')
subdir('way-cooler')
