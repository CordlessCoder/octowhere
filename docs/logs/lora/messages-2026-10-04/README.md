# Messages on two boards, 2026-10-04

The 2026-10-04 hand-off's messages, between board 1A:38 (id 0, Dredge) and 1C:1C (id 1, Roger
Roger) in their group of two. Both ran the working tree built with `fix-inject`, `touch-inject`
and `pair-inject`. `tools/fix-inject.py --mesh` gave 1C:1C GPS time from its RTC, so that the
two shared a timebase; 1A:38 took it at 404 s. 1C:1C sent through `tools/pair-inject.py send`;
1A:38 was driven through its screens with `tools/touch-inject.py`, which also read them back.
The logs are the two boards' serial captures.

| Shot | What it shows |
| --- | --- |
| `arrival.png` | 1A:38's clock face as 1C:1C's private message arrived: the toast names its sender and nothing of its words, and the unread arc is on |
| `inbox.png` | The Messages root: the conversation with Roger Roger, one unread |
| `thread.png` | The conversation, its message RECEIVED |
| `draft.png` | WRITE: the reply typed on the keyboard, REVIEW lit |
| `review.png` | The reply read through before it is sent |
| `queued.png` | The conversation just after SEND: the reply QUEUED |
| `delivered.png` | The reply DELIVERED, once Roger Roger acknowledged it |

What the run showed:

- 1C:1C queued and posted the message at 405.9 s, and it reached 1A:38 at 430.9 s
  (`1a38.log`), which showed the toast.
- The message counted as read once its row had shown whole for a second in the open
  conversation: 1A:38 logged `command Read(1)` at 470.3 s. Opening the inbox read nothing.
- 1A:38's reply went out through its screens at 531.1 s and reached 1C:1C at 545.4 s.
  1C:1C's acknowledgement reached 1A:38 at 625.0 s, and the conversation showed DELIVERED.
  No shot caught it SENT, between the first packet that carried it and the acknowledgement.
- The draft's title, TO ROGER ROGER, is wider than its 274 px at 26 px, and is cut short with
  an ellipsis. The review's caption names him in full.
- As in the earlier runs, the GNSS module's resets follow the debugger's halts for each shot,
  and the shots show pixels past the glass's edge that the panel never shows.
