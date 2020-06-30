#!/usr/bin/env bash 

# Version 0.1.1, 2020-06-30

cmdname=${0##*/}

echoerr() { 
    echo "ERR: $@" 1>&2 
}

usage()
{
    cat << USAGE >&2
Description:
    build image of service (Dockerfile in subfolder at docker/) and push to Docker Hub
Usage:
    $cmdname [--help | [SERVICE] [-b] ]
params:
    SERVICE   from docker-compose.yml, and self-titled subfolder at docker/, 'proj' by default
options:
    -b        build only, no push
    --help    this help
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

if [[ ! $service ]]; then
    service=proj
fi

bw_proj_name=$(env $(cat .env | grep -v '#' | xargs) bash -c 'echo $BW_PROJ_NAME')
if [[ ! $bw_proj_name ]]; then
    echoerr "BW_PROJ_NAME value must be specified in file'.env'"
    exit 1
fi

bw_proj_version=$(env $(cat .env | grep -v '#' | xargs) bash -c 'echo $BW_PROJ_VERSION')
if [[ ! $bw_proj_version ]]; then
    echoerr "BW_PROJ_VERSION value must be specified in file'.env'"
    exit 1
fi

gitlab_service_version=$(cat .gitlab-ci.yml | sed -nr "s/^[[:space:]]*image:[[:space:]]*bazawinner\/dev-${bw_proj_name}-$service:([^[:space:]])/\1/p;")
if [[ ! $gitlab_service_version ]]; then
    echoerr "image: dev-${bw_proj_name}-$service:VERSION not found in file'.gitlab-ci.yml'"
    exit 1
fi

if [[ ! $build_only && $bw_proj_version == $gitlab_service_version ]]; then
    echoerr "BW_PROJ_VERSION ($bw_proj_version) in file'.env' must differ (be bigger) than version ($gitlab_service_version) in file'.gitlab-ci.yml' in line: image: bazawinner/dev-${bw_proj_name}-$service:$gitlab_service_version"
    exit 1
fi

env $(cat .env | grep -v '#' | xargs) bash -c '\
    tag=bazawinner/dev-$BW_PROJ_NAME-'$service':$BW_PROJ_VERSION
    echo Building $tag . . .
    docker build '${opts[@]}' \
        $(\
            src=docker/'$service'/Dockerfile; 
            cat $src | \
            sed -nr "s/^[[:space:]]*ARG[[:space:]]+([[:alnum:]_]+)/--build-arg=\1/p"\
        ) \
        -t $tag \
        docker/'$service' && \
    echo OK: Built $tag && \
    if [[ ! "'$build_only'" ]] 
    then
        echo Pushing $tag . . .
        docker push $tag
        echo OK: Pushed $tag
    fi
'
