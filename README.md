# gmg - Super-lightweight git manager

## The idea

The idea is simple: no extra server layer, no extra proxies. Just a pure git
and ssh.

## Setup

The setup scripts are made for Debian/Ubuntu systems

* Launch a blank Debian/Ubuntu container

* gmg requires the system git 2.34 or above. So use either a modern system or
  install the newer git manually.

* Setup ssh server

* Download a gmg binary from the
    [releases](https://github.com/alttch/gmg/releases) and put it somewhere
    e.g. to */usr/local/bin/*

* Copy to the container the "share" folder from the repository

* Execute on the container (where org is your organization and org.com is your
organization domain):

```shell
cd share && ./gmg-setup git@org git@org.com
```

and that is it.

The setup creates */git* folder for repositories and configures the global
update hook to protect main branches.

## Quick start

### Creating a new repository

```
gmg repo create test -D "My test repo"
```

The repository path can contain groups. E.g. "tests/test". gmg uses repository
names as POSIX groups to manage access, so a full repository name (including
groups) can not be longer than 30 symbols.

### Creating a user

```
gmg user create bob "Bob M" -
```

Copy-paste the public ssh key-file to stdin (or use a file name instead of "-"
argument)

### Granting user access to a repository

```
gmg user grant bob test
```

### Cloning

Repositories can be cloned as

```
git clone ssh://bob@server/git/test
```

Additionally, users get symbolic links created in their homes as soon as access
has been granted:

```
git clone ssh://bob@server:test
```

### Setting user as the maintainer

Maintainers can write to the main branch, for others it is forbidden.

```
gmg maintainer set bob test
```

### Other operations

Type

```
gmg -h
```

for all possible commands.

## Integrating with cgit

gmg automatically generates cgit-compatible configs.

* Install cgit and NGINX

```
apt -y install cgit nginx fcgiwrap
```

* Put share/cgit-gmg.cgi into */usr/local/bin/*

* Use the following NGINX config:

```
server {
    listen                80;
    rewrite ^/(.*)/$ https://your-external.domain/$1 permanent;
    rewrite ^/cgit-css/(.*) /$1 last;
    root                  /usr/share/cgit;
    try_files             $uri @cgit;

    auth_pam              "Git";
    auth_pam_service_name "nginx";

    location @cgit {
      include             fastcgi_params;
      fastcgi_param       SCRIPT_FILENAME /usr/local/bin/cgit-gmg.cgi;
      fastcgi_param       PATH_INFO       $uri;
      fastcgi_param       QUERY_STRING    $args;
      fastcgi_param       HTTP_HOST       $server_name;
      fastcgi_pass        unix:/var/run/fcgiwrap.socket;
    }
}
```

* To let users use cgit, they must have system passwords set
