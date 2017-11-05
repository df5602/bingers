# bingers
Manage your TV shows from the command line

_Note_: This is a personal side-project just for fun and still a work in progress..

## Usage
### Subscribe to shows
```
$ ./target/debug/bingers add orville
Found:

	The Orville (Thursdays on FOX, 60')

Add show? [y (yes); n (no); a (abort)] y
Added "The Orville"

Have you already watched some episodes of The Orville? [y (yes); n (no)] y
Season | Episode | Name                       | Air Date
-------|---------|----------------------------|-------------------
     1 |       1 | Old Wounds                 | Mon, Sep 11, 2017
     1 |       2 | Command Performance        | Mon, Sep 18, 2017
     1 |       3 | About a Girl               | Fri, Sep 22, 2017
     1 |       4 | If the Stars Should Appear | Fri, Sep 29, 2017
     1 |       5 | Pria                       | Fri, Oct 06, 2017
     1 |       6 | Krill                      | Fri, Oct 13, 2017
     1 |       7 | Majority Rule              | Fri, Oct 27, 2017
     1 |       8 | Into the Fold              | Fri, Nov 03, 2017

Specify the last episode you have watched:
Season: 1
Episode: 7
$
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
