Python bindings for [`container_image_dist_ref`](https://github.com/skalt/container_image_dist_ref),
a fast, dependency-free docker/OCI image reference parser.

```py
import container_image_dist_ref as cidr

ref = cidr.parse("docker.io/library/alpine:3.14@sha256:" + "a" * 64)
print(ref.name)          # docker.io/library/alpine
print(str(ref.domain))   # docker.io
print(str(ref.path))     # library/alpine
print(str(ref.tag))      # 3.14
print(str(ref.digest))   # sha256:aaa...
```
