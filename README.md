# `gitt`

`gitt` is a clone of `gitk` that runs in your terminal.

# Usage

`gitt` with no parameters will show you the history of the current branch in the current directory.

When provided with a commit id or branch name `gitt` will show the history from that tree.

Use the arrow keys or `j` and `k` to scroll the list or diff, and `tab` to switch the focus between the list and diff.

`g` and `G` scrolls to the top and bottom of the focussed area.

`q` terminates `gitt`.

# Motivation

`gitk` is an underrated tool and a big improvement over `git log`. However, it is generally invoked from a terminal and on tiling window managers this means wasted screen real estate for the now unused terminal. The UI elements are frequently incorrectly sized, more-so than simply due to screen real estate changing, but entire columns truncated or the diff pushed off the edge of the window. Finally, copying and pasting SHA1s requires leaving the application open (I will often copy the SHA1 into a random terminal in order to preserve it after I close `gitk`).

`gitt` is a useful learning experience for myself and nicely addresses my main pain points when using `gitk`.

`gitt` is intended to be a `gitk` clone, and not a tool to help with other `git` workflows. For example [`gitui`](https://github.com/extrawurst/gitui) includes tools to help with staging changes. This is intentionally out of scope for `gitt`.
