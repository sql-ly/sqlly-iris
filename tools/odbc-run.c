#include <sql.h>
#include <sqlext.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void diagnostics(SQLHANDLE handle, SQLSMALLINT type, const char *operation) {
    SQLSMALLINT record = 1;
    SQLCHAR state[6] = {0};
    SQLINTEGER native = 0;
    SQLCHAR message[2048];
    SQLSMALLINT length = 0;
    fprintf(stderr, "%s failed\n", operation);
    while (SQLGetDiagRec(type, handle, record, state, &native, message, sizeof(message), &length) != SQL_NO_DATA) {
        fprintf(stderr, "  %s/%d: %s\n", state, (int)native, message);
        record++;
    }
}

static int dump_results(HSTMT statement) {
    int result_index = 0;
    for (;;) {
        SQLSMALLINT columns = 0;
        SQLLEN row_count = 0;
        SQLRETURN rc = SQLNumResultCols(statement, &columns);
        if (!SQL_SUCCEEDED(rc)) {
            diagnostics(statement, SQL_HANDLE_STMT, "SQLNumResultCols");
            return 1;
        }
        SQLRowCount(statement, &row_count);
        printf("result=%d columns=%d row_count=%lld\n", result_index, (int)columns,
               (long long)row_count);
        if (columns > 0) {
            SQLSMALLINT index;
            for (index = 1; index <= columns; index++) {
                SQLCHAR name[256];
                SQLSMALLINT length = 0, type = 0, digits = 0, nullable = 0;
                SQLULEN size = 0;
                rc = SQLDescribeCol(statement, index, name, sizeof(name), &length, &type, &size,
                                    &digits, &nullable);
                if (!SQL_SUCCEEDED(rc)) {
                    diagnostics(statement, SQL_HANDLE_STMT, "SQLDescribeCol");
                    return 1;
                }
                printf("%s%s", index == 1 ? "" : "\t", name);
            }
            putchar('\n');
            for (;;) {
                rc = SQLFetch(statement);
                if (rc == SQL_NO_DATA)
                    break;
                if (!SQL_SUCCEEDED(rc)) {
                    diagnostics(statement, SQL_HANDLE_STMT, "SQLFetch");
                    return 1;
                }
                for (index = 1; index <= columns; index++) {
                    SQLCHAR value[4096];
                    SQLLEN length = 0;
                    rc = SQLGetData(statement, index, SQL_C_CHAR, value, sizeof(value), &length);
                    if (!SQL_SUCCEEDED(rc)) {
                        diagnostics(statement, SQL_HANDLE_STMT, "SQLGetData");
                        return 1;
                    }
                    printf("%s%s", index == 1 ? "" : "\t",
                           length == SQL_NULL_DATA ? "<null>" : (char *)value);
                }
                putchar('\n');
            }
        }
        rc = SQLMoreResults(statement);
        if (rc == SQL_NO_DATA)
            break;
        if (!SQL_SUCCEEDED(rc)) {
            diagnostics(statement, SQL_HANDLE_STMT, "SQLMoreResults");
            return 1;
        }
        result_index++;
    }
    return 0;
}

int main(int argc, char **argv) {
    const char *connection_string = getenv("IRIS_ODBC_CONNECTION_STRING");
    SQLHENV environment = SQL_NULL_HENV;
    SQLHDBC connection = SQL_NULL_HDBC;
    SQLHSTMT statement = SQL_NULL_HSTMT;
    SQLRETURN rc;
    int autocommit_off = 0;
    SQLULEN isolation = 0;
    SQLSMALLINT completion = -1;
    int result = 1;
    int index;
    if (!connection_string) {
        fputs("Set IRIS_ODBC_CONNECTION_STRING.\n", stderr);
        return 2;
    }
    rc = SQLAllocHandle(SQL_HANDLE_ENV, SQL_NULL_HANDLE, &environment);
    if (!SQL_SUCCEEDED(rc))
        return 1;
    SQLSetEnvAttr(environment, SQL_ATTR_ODBC_VERSION, (SQLPOINTER)SQL_OV_ODBC3, 0);
    rc = SQLAllocHandle(SQL_HANDLE_DBC, environment, &connection);
    if (!SQL_SUCCEEDED(rc))
        goto done;
    rc = SQLDriverConnect(connection, NULL, (SQLCHAR *)connection_string, SQL_NTS, NULL, 0,
                          NULL, SQL_DRIVER_NOPROMPT);
    if (!SQL_SUCCEEDED(rc)) {
        diagnostics(connection, SQL_HANDLE_DBC, "SQLDriverConnect");
        goto done;
    }
    rc = SQLAllocHandle(SQL_HANDLE_STMT, connection, &statement);
    if (!SQL_SUCCEEDED(rc))
        goto done;
    for (index = 1; index < argc; index++) {
        if (!strcmp(argv[index], "--autocommit-off")) {
            autocommit_off = 1;
        } else if (!strcmp(argv[index], "--read-committed")) {
            isolation = SQL_TXN_READ_COMMITTED;
        } else if (!strcmp(argv[index], "--read-uncommitted")) {
            isolation = SQL_TXN_READ_UNCOMMITTED;
        } else if (!strcmp(argv[index], "--commit")) {
            completion = SQL_COMMIT;
        } else if (!strcmp(argv[index], "--rollback")) {
            completion = SQL_ROLLBACK;
        }
    }
    if (autocommit_off) {
        rc = SQLSetConnectAttr(connection, SQL_ATTR_AUTOCOMMIT,
                               (SQLPOINTER)SQL_AUTOCOMMIT_OFF, 0);
        if (!SQL_SUCCEEDED(rc)) {
            diagnostics(connection, SQL_HANDLE_DBC, "SQLSetConnectAttr(AUTOCOMMIT_OFF)");
            goto done;
        }
    }
    if (isolation) {
        rc = SQLSetConnectAttr(connection, SQL_ATTR_TXN_ISOLATION, (SQLPOINTER)isolation, 0);
        if (!SQL_SUCCEEDED(rc)) {
            diagnostics(connection, SQL_HANDLE_DBC, "SQLSetConnectAttr(TXN_ISOLATION)");
            goto done;
        }
    }
    for (index = 1; index < argc; index++) {
        if (!strcmp(argv[index], "--autocommit-off") ||
            !strcmp(argv[index], "--read-committed") ||
            !strcmp(argv[index], "--read-uncommitted") || !strcmp(argv[index], "--commit") ||
            !strcmp(argv[index], "--rollback"))
            continue;
        printf("sql=%s\n", argv[index]);
        rc = SQLExecDirect(statement, (SQLCHAR *)argv[index], SQL_NTS);
        if (!SQL_SUCCEEDED(rc)) {
            diagnostics(statement, SQL_HANDLE_STMT, "SQLExecDirect");
            goto done;
        }
        if (dump_results(statement))
            goto done;
        rc = SQLFreeStmt(statement, SQL_CLOSE);
        if (!SQL_SUCCEEDED(rc))
            goto done;
    }
    if (completion >= 0) {
        rc = SQLEndTran(SQL_HANDLE_DBC, connection, completion);
        if (!SQL_SUCCEEDED(rc)) {
            diagnostics(connection, SQL_HANDLE_DBC, "SQLEndTran");
            goto done;
        }
    }
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
