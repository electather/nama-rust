# Servers trust the Hub through device-flow Linking and Hub-signed tokens

A Server is registered with the Hub, and a User is Linked to an Account, through an OAuth device flow: the Server shows a code and the person enters it on the Hub. The Hub signs short-lived JWTs with an asymmetric key. Servers verify them against the Hub's cached public key set, so a client arriving through the Hub is authenticated even if the Hub is briefly unreachable, and the Hub never needs inbound access to a Server.
