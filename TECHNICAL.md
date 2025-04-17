# App entry point
main.rs
Using clap module to have a command-line application
## 3 commands
### Run
start and run the activity tracker in the background.
##### The logic
While Loop
Continuously check the app under the cursor
If changed is detected-> an interval is recorded in the DB.

### Stats
generate a start report about app usage for:
-current hour
-last hour
-current day
### Update
Check the last availabel version of the app available
# What is recorded
We record the position of the mouse every 1 second.-> all those intervals are registered in the DB.
We later merge them in order to avoid having too much data in the DB.
# Where 
in an SQLite DB
## Data registered

# Current limitation
-Update can introduce breaking changes. Specially in the DB. 
-Migration are not currently tracked on the DB. A new version can oblige you to delete your current DB and have a new DB created the first time you rerun the new app.


# Environment 
## Dev
