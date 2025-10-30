# ewwvolume

volume control visualizer widget for eww

## dependencies
eww, wireplumber

## installation

1. install the binary:
```
   cargo install --path .
```

2. run the install script:
```
   bash install.sh
```

3. add this to your eww.yuck:
```
   (include "./ewwvolume/ewwvolume.yuck")
```

## setup
the ewwvolume binary is designed to be spawned on key events, it may be convenient to do this through desktop environment or compositor configuration.

## arguments
the binary must be spawned with an argument
`up` increase volume 3%
`down` decrease volume 3%
`mute-toggle` toggle output mute
`mic-toggle` toggle input mute