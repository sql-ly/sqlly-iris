#include <sql.h>
#include <sqlext.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CHECK(handle, type, call)                     \
    do {                                              \
        SQLRETURN check_rc = (call);                  \
        if (!SQL_SUCCEEDED(check_rc)) {               \
            print_diagnostics((handle), (type), #call); \
            return 1;                                 \
        }                                             \
    } while (0)

typedef struct {
    SQLUSMALLINT id;
    const char *name;
} FunctionEntry;

static void print_diagnostics(SQLHANDLE handle, SQLSMALLINT type, const char *operation) {
    SQLSMALLINT record = 1;
    SQLCHAR state[6] = {0};
    SQLINTEGER native = 0;
    SQLCHAR message[2048];
    SQLSMALLINT message_length = 0;
    fprintf(stderr, "%s failed\n", operation);
    while (SQLGetDiagRec(type, handle, record, state, &native, message,
                         (SQLSMALLINT)sizeof(message), &message_length) != SQL_NO_DATA) {
        fprintf(stderr, "  %s/%d: %s\n", state, (int)native, message);
        record++;
    }
}

static int print_functions(SQLHDBC connection) {
#define FUNCTION(id) {id, #id}
    static const FunctionEntry functions[] = {
        FUNCTION(SQL_API_SQLALLOCCONNECT), FUNCTION(SQL_API_SQLALLOCENV),
        FUNCTION(SQL_API_SQLALLOCHANDLE), FUNCTION(SQL_API_SQLALLOCSTMT),
        FUNCTION(SQL_API_SQLBINDCOL), FUNCTION(SQL_API_SQLBINDPARAMETER),
        FUNCTION(SQL_API_SQLCANCEL), FUNCTION(SQL_API_SQLCLOSECURSOR),
        FUNCTION(SQL_API_SQLCOLATTRIBUTE), FUNCTION(SQL_API_SQLCOLUMNPRIVILEGES),
        FUNCTION(SQL_API_SQLCOLUMNS), FUNCTION(SQL_API_SQLCONNECT),
        FUNCTION(SQL_API_SQLDESCRIBECOL), FUNCTION(SQL_API_SQLDESCRIBEPARAM),
        FUNCTION(SQL_API_SQLDISCONNECT), FUNCTION(SQL_API_SQLDRIVERCONNECT),
        FUNCTION(SQL_API_SQLENDTRAN), FUNCTION(SQL_API_SQLERROR),
        FUNCTION(SQL_API_SQLEXECDIRECT), FUNCTION(SQL_API_SQLEXECUTE),
        FUNCTION(SQL_API_SQLEXTENDEDFETCH), FUNCTION(SQL_API_SQLFETCH),
        FUNCTION(SQL_API_SQLFETCHSCROLL), FUNCTION(SQL_API_SQLFOREIGNKEYS),
        FUNCTION(SQL_API_SQLFREECONNECT), FUNCTION(SQL_API_SQLFREEENV),
        FUNCTION(SQL_API_SQLFREEHANDLE), FUNCTION(SQL_API_SQLFREESTMT),
        FUNCTION(SQL_API_SQLGETCONNECTATTR), FUNCTION(SQL_API_SQLGETCONNECTOPTION),
        FUNCTION(SQL_API_SQLGETCURSORNAME), FUNCTION(SQL_API_SQLGETDATA),
        FUNCTION(SQL_API_SQLGETDIAGFIELD), FUNCTION(SQL_API_SQLGETDIAGREC),
        FUNCTION(SQL_API_SQLGETENVATTR), FUNCTION(SQL_API_SQLGETFUNCTIONS),
        FUNCTION(SQL_API_SQLGETINFO), FUNCTION(SQL_API_SQLGETSTMTATTR),
        FUNCTION(SQL_API_SQLGETSTMTOPTION), FUNCTION(SQL_API_SQLGETTYPEINFO),
        FUNCTION(SQL_API_SQLMORERESULTS), FUNCTION(SQL_API_SQLNATIVESQL),
        FUNCTION(SQL_API_SQLNUMPARAMS), FUNCTION(SQL_API_SQLNUMRESULTCOLS),
        FUNCTION(SQL_API_SQLPARAMDATA), FUNCTION(SQL_API_SQLPARAMOPTIONS),
        FUNCTION(SQL_API_SQLPREPARE), FUNCTION(SQL_API_SQLPRIMARYKEYS),
        FUNCTION(SQL_API_SQLPROCEDURECOLUMNS), FUNCTION(SQL_API_SQLPROCEDURES),
        FUNCTION(SQL_API_SQLPUTDATA), FUNCTION(SQL_API_SQLROWCOUNT),
        FUNCTION(SQL_API_SQLSETCONNECTATTR), FUNCTION(SQL_API_SQLSETCONNECTOPTION),
        FUNCTION(SQL_API_SQLSETCURSORNAME), FUNCTION(SQL_API_SQLSETENVATTR),
        FUNCTION(SQL_API_SQLSETPARAM), FUNCTION(SQL_API_SQLSETSCROLLOPTIONS),
        FUNCTION(SQL_API_SQLSETSTMTATTR), FUNCTION(SQL_API_SQLSETSTMTOPTION),
        FUNCTION(SQL_API_SQLSPECIALCOLUMNS), FUNCTION(SQL_API_SQLSTATISTICS),
        FUNCTION(SQL_API_SQLTABLEPRIVILEGES), FUNCTION(SQL_API_SQLTABLES),
        FUNCTION(SQL_API_SQLTRANSACT),
    };
#undef FUNCTION
    size_t index;
    puts("[functions]");
    for (index = 0; index < sizeof(functions) / sizeof(functions[0]); index++) {
        SQLUSMALLINT supported = SQL_FALSE;
        CHECK(connection, SQL_HANDLE_DBC,
              SQLGetFunctions(connection, functions[index].id, &supported));
        printf("%s=%s\n", functions[index].name, supported ? "yes" : "no");
    }
    return 0;
}

static int print_info_string(SQLHDBC connection, SQLUSMALLINT type, const char *name) {
    SQLCHAR value[8192];
    SQLSMALLINT length = 0;
    SQLRETURN rc = SQLGetInfo(connection, type, value, sizeof(value), &length);
    if (!SQL_SUCCEEDED(rc)) {
        printf("%s=<unavailable:%d>\n", name, (int)rc);
        return 0;
    }
    printf("%s=%.*s\n", name, (int)length, value);
    return 0;
}

static int print_info_u32(SQLHDBC connection, SQLUSMALLINT type, const char *name) {
    SQLUINTEGER value = 0;
    SQLSMALLINT length = 0;
    SQLRETURN rc = SQLGetInfo(connection, type, &value, sizeof(value), &length);
    if (!SQL_SUCCEEDED(rc)) {
        printf("%s=<unavailable:%d>\n", name, (int)rc);
        return 0;
    }
    printf("%s=%u\n", name, (unsigned int)value);
    return 0;
}

static int print_info(SQLHDBC connection) {
    puts("[info]");
#define INFO_STRING(id)                         \
    do {                                        \
        if (print_info_string(connection, id, #id)) \
            return 1;                           \
    } while (0)
#define INFO_U32(id)                         \
    do {                                     \
        if (print_info_u32(connection, id, #id)) \
            return 1;                        \
    } while (0)
    INFO_STRING(SQL_DRIVER_NAME);
    INFO_STRING(SQL_DRIVER_VER);
    INFO_STRING(SQL_DRIVER_ODBC_VER);
    INFO_STRING(SQL_DBMS_NAME);
    INFO_STRING(SQL_DBMS_VER);
    INFO_STRING(SQL_DATA_SOURCE_NAME);
    INFO_STRING(SQL_DATABASE_NAME);
    INFO_STRING(SQL_SERVER_NAME);
    INFO_STRING(SQL_USER_NAME);
    INFO_STRING(SQL_IDENTIFIER_QUOTE_CHAR);
    INFO_STRING(SQL_KEYWORDS);
    INFO_STRING(SQL_SPECIAL_CHARACTERS);
    INFO_STRING(SQL_ODBC_SQL_OPT_IEF);
    INFO_U32(SQL_ODBC_API_CONFORMANCE);
    INFO_U32(SQL_ODBC_SQL_CONFORMANCE);
    INFO_U32(SQL_SQL_CONFORMANCE);
    INFO_U32(SQL_CURSOR_COMMIT_BEHAVIOR);
    INFO_U32(SQL_CURSOR_ROLLBACK_BEHAVIOR);
    INFO_U32(SQL_TXN_CAPABLE);
    INFO_U32(SQL_TXN_ISOLATION_OPTION);
    INFO_U32(SQL_DEFAULT_TXN_ISOLATION);
    INFO_U32(SQL_SCROLL_OPTIONS);
    INFO_U32(SQL_STATIC_CURSOR_ATTRIBUTES1);
    INFO_U32(SQL_STATIC_CURSOR_ATTRIBUTES2);
    INFO_U32(SQL_FORWARD_ONLY_CURSOR_ATTRIBUTES1);
    INFO_U32(SQL_FORWARD_ONLY_CURSOR_ATTRIBUTES2);
    INFO_U32(SQL_DYNAMIC_CURSOR_ATTRIBUTES1);
    INFO_U32(SQL_DYNAMIC_CURSOR_ATTRIBUTES2);
    INFO_U32(SQL_KEYSET_CURSOR_ATTRIBUTES1);
    INFO_U32(SQL_KEYSET_CURSOR_ATTRIBUTES2);
    INFO_U32(SQL_MAX_IDENTIFIER_LEN);
    INFO_U32(SQL_MAX_TABLE_NAME_LEN);
    INFO_U32(SQL_MAX_COLUMN_NAME_LEN);
    INFO_U32(SQL_MAX_SCHEMA_NAME_LEN);
    INFO_U32(SQL_MAX_CATALOG_NAME_LEN);
    INFO_U32(SQL_MAX_COLUMNS_IN_TABLE);
    INFO_U32(SQL_MAX_COLUMNS_IN_SELECT);
    INFO_U32(SQL_MAX_COLUMNS_IN_ORDER_BY);
    INFO_U32(SQL_MAX_COLUMNS_IN_GROUP_BY);
    INFO_U32(SQL_MAX_STATEMENT_LEN);
    INFO_U32(SQL_MAX_ROW_SIZE);
    INFO_U32(SQL_MAX_CHAR_LITERAL_LEN);
    INFO_U32(SQL_MAX_BINARY_LITERAL_LEN);
    INFO_U32(SQL_PARAM_ARRAY_ROW_COUNTS);
    INFO_U32(SQL_PARAM_ARRAY_SELECTS);
    INFO_U32(SQL_BATCH_SUPPORT);
    INFO_U32(SQL_BATCH_ROW_COUNT);
    INFO_STRING(SQL_MULT_RESULT_SETS);
    INFO_STRING(SQL_MULTIPLE_ACTIVE_TXN);
    INFO_STRING(SQL_OUTER_JOINS);
#undef INFO_STRING
#undef INFO_U32
    return 0;
}

static int print_type_info(SQLHSTMT statement) {
    SQLSMALLINT columns = 0;
    SQLSMALLINT index;
    puts("[type_info]");
    CHECK(statement, SQL_HANDLE_STMT, SQLGetTypeInfo(statement, SQL_ALL_TYPES));
    CHECK(statement, SQL_HANDLE_STMT, SQLNumResultCols(statement, &columns));
    for (index = 1; index <= columns; index++) {
        SQLCHAR name[256];
        SQLSMALLINT length = 0;
        SQLSMALLINT type = 0;
        SQLULEN size = 0;
        SQLSMALLINT digits = 0;
        SQLSMALLINT nullable = 0;
        CHECK(statement, SQL_HANDLE_STMT,
              SQLDescribeCol(statement, index, name, sizeof(name), &length, &type, &size,
                             &digits, &nullable));
        printf("%s%s", index == 1 ? "" : "\t", name);
    }
    putchar('\n');
    for (;;) {
        SQLRETURN rc = SQLFetch(statement);
        if (rc == SQL_NO_DATA)
            break;
        if (!SQL_SUCCEEDED(rc)) {
            print_diagnostics(statement, SQL_HANDLE_STMT, "SQLFetch(SQLGetTypeInfo)");
            return 1;
        }
        for (index = 1; index <= columns; index++) {
            SQLCHAR value[4096];
            SQLLEN length = 0;
            rc = SQLGetData(statement, index, SQL_C_CHAR, value, sizeof(value), &length);
            if (!SQL_SUCCEEDED(rc)) {
                print_diagnostics(statement, SQL_HANDLE_STMT, "SQLGetData(SQLGetTypeInfo)");
                return 1;
            }
            printf("%s%s", index == 1 ? "" : "\t",
                   length == SQL_NULL_DATA ? "<null>" : (char *)value);
        }
        putchar('\n');
    }
    return 0;
}

int main(void) {
    const char *connection_string = getenv("IRIS_ODBC_CONNECTION_STRING");
    SQLHENV environment = SQL_NULL_HENV;
    SQLHDBC connection = SQL_NULL_HDBC;
    SQLHSTMT statement = SQL_NULL_HSTMT;
    int result = 1;
    if (connection_string == NULL || connection_string[0] == '\0') {
        fputs("Set IRIS_ODBC_CONNECTION_STRING to a DSN-less ODBC connection string.\n", stderr);
        return 2;
    }
    CHECK(SQL_NULL_HANDLE, 0, SQLAllocHandle(SQL_HANDLE_ENV, SQL_NULL_HANDLE, &environment));
    CHECK(environment, SQL_HANDLE_ENV,
          SQLSetEnvAttr(environment, SQL_ATTR_ODBC_VERSION, (SQLPOINTER)SQL_OV_ODBC3, 0));
    CHECK(environment, SQL_HANDLE_ENV,
          SQLAllocHandle(SQL_HANDLE_DBC, environment, &connection));
    CHECK(connection, SQL_HANDLE_DBC,
          SQLDriverConnect(connection, NULL, (SQLCHAR *)connection_string, SQL_NTS, NULL, 0,
                           NULL, SQL_DRIVER_NOPROMPT));
    CHECK(connection, SQL_HANDLE_DBC,
          SQLAllocHandle(SQL_HANDLE_STMT, connection, &statement));
    if (print_functions(connection) || print_info(connection) || print_type_info(statement))
        goto done;
    result = 0;
done:
    if (statement != SQL_NULL_HSTMT)
        SQLFreeHandle(SQL_HANDLE_STMT, statement);
    if (connection != SQL_NULL_HDBC) {
        SQLDisconnect(connection);
        SQLFreeHandle(SQL_HANDLE_DBC, connection);
    }
    if (environment != SQL_NULL_HENV)
        SQLFreeHandle(SQL_HANDLE_ENV, environment);
    return result;
}
