# easy-fs-fuse

- To facilitate easy testing/debugging of easy-fs operation, easy-fs testing using the standard library std is performed on the Linux side in this workspace.

- In Chapter 6, all applications had to be linked to the kernel and then indexed by application name in the application manager to find the ELF data for the application. This has the disadvantage of overSizing the kernel.

Unexecuted applications also occupy memory space, thus wasting memory resources.

With the implementation of the easy-fs file system, it is finally possible to package these applications into easy-fs images on disk, so that when you want to run an application, you simply pull the ELF executable from the file system and load it into memory So we can now avoid the storage overhead of the previous chapter.
