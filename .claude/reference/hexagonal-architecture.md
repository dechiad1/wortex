## Hexagonal Architecture

#### Summary 
The package structure should follow the hexagonal pattern with the following top level directories: api, domain, adapters. Application configuration, entry point & dependency injection are handled at the top layer
##### API package
This is our driving adapter package. 

api: this package should include the code needed for the "input" to the application. This can take the form of http requests, command line parameters, reading messages off of a queue. since we will always start with an API, we create our fastAPI app & routers here (& then import them in main in the application entry point). this layer should contain concerns related to the input technology (importing from fastAPI library, handling http concerns) & can import types from domain. 
- the schemas package
- the mappers package
- the routers package (we start with this). the routers serve an http API with fastAPI & abide by openapi.json specification
- the cli package may be added in the future & will implement schemas, mappers as well. 
Error handling is done in this layer, specific to the medium. For example, we map domain errors to http errors for our routers package. 

##### Domain package
domain: this package should include services, models & ports. 
- the **models** are objects that inherit from pydantic basemodel & have methods local to their functionality & ensure their invariance
	- ORM models do not belong in the domain
- the **ports** package contains interfaces implementing ABC so that multiple implementations can be defined in adapters. for example, we might start out using postgres in a local contain & then deploy to supabase. any changes we encounter will only occur in the adapter package.  
- the **services** will contain business logic that support CRUD operations or usecases that act on models in the domain. The business logic here are simple CRUD operations on domain models. If an operation requires modifying multiple entities we should be creating a command to encapsulate this functionality
- the **commands** package will contain complex business logic that spans multiple domains. For example, if a post has multiple comments the "delete post" functionality must be a command that encapsulates the functionality to find all comments associated to the post, delete them & then delete the post
- any sub package in domain can never import any types/dependencies from other top level packages. It should strictly be importing other domain packages - use the interface defined in the port for side effect functionality. 

unit tests should be written for the domain.models, the domain.services & the domain.commands packages. 
##### Adapters package
adapters: this package should include implementations of ports that handle outbound connectivity & side effects. Examples include: database interaction, object storage management, connection to a third party API, connection to an LLM, connection to cloud services of any sort. These implementations must adhere to a port so that they can be easily swapped out without impacting the domain. Anything that is specific to the technology being implemented must be isolated to this package. 

The database explicitly belongs in the adapters package. a port allows us to easily switch between databases for various environments. for this reason, we prefer to implement direct sql based interactions with databases as opposed to using ORMs. 
#### Additional considerations
The application needs a few things that will live at the top level & do not go into the packages. These include application configuration & dependency injection. 

application configuration (environment): there should be a configuration file for each environment - local, dev, production in a top level directory titled 'config'. the proper configuration file selection is environment variable driven, selecting the file with a '-env' (-dev, -prod) suffix & defaults to the local profile if no environment variable is found. the env contents are parsed into a configuration object at start up that can be used by dependency injection. 

dependency injection: is done in a file called dependencies.py at the root of the application. When the application starts, we will call functions to return dependencies instantiated in dependencies.py in the main.py file. the main.py file will perform the orchestration & set up of the application: creating the routers, creating dependencies & injecting them where needed. Finally, main.py will start the http server with uvicorn. 
#### Enforcement
Its important that we can have checks in place so that all new additions to the code base pass architectural regression. We'll do this with a hexagonal layering rule in our linter. 
```
[[tool.importlinter.contracts]]
name = "Hexagonal layers" 
type = "layers" 
layers = [ 
	"myapp.api", 
	"myapp.domain",
	"myapp.adapters",
]
```
We want to ensure that API can import domain, adapters can import domain & domain can not import anything. 