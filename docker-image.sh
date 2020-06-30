#!/usr/bin/env bash 

# Version 2.0.0, 2020-07-01

cmdname=${0##*/}

echoerr() { 
    echo "ERR: $@" 1>&2 
}

usage()
{
    cat << USAGE >&2
Description:
    build image of service (Dockerfile in subfolder at ./TARGET/) and push to Docker Hub
Usage:
    $cmdname [--help | [SERVICE] [-b] ]
params:
    TARGET      'dev' or 'prod', 'dev' by default
    SERVICE     from self-titled subfolder at ./TARGET/, 'proj' by default
options:
    -b          build only, no push
    --help      this help
    --no-cache  pass '--no-cache' to 'docker build'
USAGE
    exit 1
}

build_only=
service=
opts=()

# process arguments
while [[ $# -gt 0 ]]
do
    case "$1" in
        -b)
            build_only=1
            shift 1
        ;;

        --help)
            usage
        ;;

        --no-cache )
            opts+=($1)
            shift 1
        ;;

        -* )
            echoerr "Unknown option: $1"
            usage
        ;;

        dev | prod) 
            if [[ $target == "" ]]; then
                target=$1
            else 
                echoerr "TARGET must be specified only once, but two found: '$target' and '$1'"
                usage
            fi
            shift 1
        ;;

        *)
            if [[ $service == "" ]]; then
                service=$1
            else 
                echoerr "SERVICE must be specified only once, but two found: '$service' and '$1'"
                usage
            fi
            shift 1
        ;;
    esac
done

if [[ ! $target ]]; then
    target=dev
fi
TARGET=${target^^}

if [[ ! $service ]]; then
    service=proj
fi
SERVICE=${service^^}

bw_version=$(env $(cat .env | grep -v '#' | xargs) bash -c 'echo $BW_'$TARGET'_'$SERVICE'_VERSION')
if [[ ! $bw_version ]]; then
    echoerr "BW_${TARGET}_${SERVICE}_VERSION value must be specified in file'.env'"
    exit 1
fi

version_fspec="$target.yml"
did_version=$(cat "$version_fspec" | sed -nr "s/^$service:[[:space:]]*([^[:space:]])/\1/p;")
if [[ ! $did_version ]]; then
    echoerr "$service: VERSION not found in file'$version_fspec'"
    exit 1
fi

if [[ ! $build_only && $bw_version == $did_version ]]; then
    echoerr "BW_${TARGET}_${SERVICE}_VERSION ($bw_version) in file'.env' must differ (be bigger) than version ($did_version) in file'$version_fspec' in line: $service: $did_version"
    exit 1
fi

env $(cat .env | grep -v '#' | xargs) bash -c '\
    tag=bazawinner/'$target'-$BW_PROJ_NAME-'$service':$BW_'$TARGET'_'$SERVICE'_VERSION
    echo Building $tag . . .
    docker build '${opts[@]}' \
        $(\
            src='$target'/'$service'/Dockerfile; 
            cat $src | \
            sed -nr "s/^[[:space:]]*ARG[[:space:]]+([[:alnum:]_]+)/--build-arg=\1/p"\
        ) \
        -t $tag \
        '$target'/'$service' && \
    echo OK: Built $tag && \
    if [[ ! "'$build_only'" ]] 
    then
        echo Pushing $tag . . .
        docker push $tag
        echo OK: Pushed $tag
    fi
'
