# Servers trust the Hub through device-flow Linking and Hub-signed tokens

_Amended by [ADR 0105](0105-the-hub-serves-the-web-app-for-all-of-an-accounts-servers.md): a Server with a browser at hand Links through a redirect to the Hub, not a code._

A Server is registered with the Hub, and a User is Linked to an Account, through an OAuth device flow: the Server shows a code and the person enters it on the Hub. The Hub signs short-lived JWTs with an asymmetric key. Servers verify them against the Hub's cached public key set, so a client arriving through the Hub is authenticated even if the Hub is briefly unreachable, and the Hub never needs inbound access to a Server.
