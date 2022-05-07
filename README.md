# Hook Me

A discord bot and web api to handle webhooks from servers like github and Gitea, with the ability to manage who has permission to use it.

![Image of working bot funcionality](https://github.com/sarahgreywolf/HookMe/raw/main/assets/preview.png)

Var Name            | Type   | Usage
--------------------|--------|--------
DISCORD_TOKEN       | String | The token for your discord bot
BOT_PREFIX          | Char   | The prefix character for bot commands
ADMIN_ROLE_ID       | String | The Role Id of the discord role you want to handle approvals
GENERAL_ROLE_ID     | String | The Role Id of the discord role for users allowed to make requests
APPROVAL_CHANNEL_ID | String | The channel id of the discord channel that requests are sent to for approval
HOOK_CHANNEL_ID     | String | The channel id of the discord channel that receives the hook messages
LOCAL_IP            | String | The servers local ip, the one you port forward to E.G 192.168.0.22
PORT                | String | The port to listen on
HOOK_ADDRESS        | String | The api's web address, can be just your external ip with port prefixed by http
MONGO_USERNAME      | String | Your mongodb database username
MONGO_PASSWORD      | String | Your mongodb database password
MONGO_ADDR          | String | The address/ip of your mongodb server
MONGO_DB            | String | The name of the database that holds HookMe's data
