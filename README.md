# bingers
Manage your TV shows from the command line

_Note_: This is a personal side-project just for fun and still a work in progress..

## Usage
### Subscribe to shows
```
$ ./target/debug/bingers add "boardwalk empire"
Found:

	Boardwalk Empire (HBO (Ended), 60')

Add show? [y (yes); n (no); a (abort)] y
Added "Boardwalk Empire"
```
### Unsubscribe to shows
```
$ ./target/debug/bingers remove "walking dead"
Removed "The Walking Dead (Sundays on AMC, 60')"
```
### List shows
```
# List all shows
$ ./target/debug/bingers list
```
### Show help
```
$ ./target/debug/bingers --help
```
## Credits
Uses TV data provided by TVmaze.com.
